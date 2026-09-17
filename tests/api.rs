//! End-to-end API tests against the file backend.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use harmony::storage::Storage;
use harmony::storage::file::FileStorage;
use harmony::{AppState, router};

struct TestApp {
    app: Router,
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

async fn test_app() -> TestApp {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("harmony.json");
    let storage = Storage::File(FileStorage::new(&path));
    let (data, etag) = storage.load().await.unwrap();
    let state = Arc::new(AppState::new(data, etag, storage));
    TestApp {
        app: router(state),
        _dir: dir,
        path,
    }
}

impl TestApp {
    async fn call(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        let body = match body {
            Some(v) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::String(
                String::from_utf8_lossy(&bytes).into_owned(),
            ))
        };
        (status, value)
    }
}

#[tokio::test]
async fn full_workflow() {
    let t = test_app().await;

    // Empty state.
    let (status, state) = t.call("GET", "/api/state", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["sessions"].as_array().unwrap().len(), 0);
    assert!(state["active_session_id"].is_null());

    // Start untagged.
    let (status, state) = t.call("POST", "/api/sessions/start", None).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let session_id = state["active_session_id"].as_str().unwrap().to_string();
    assert!(state["sessions"][0]["project_id"].is_null());

    // Starting again conflicts.
    let (status, _) = t.call("POST", "/api/sessions/start", Some(json!({}))).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Create a project (auto colour), then tag the running session.
    let (status, created) = t
        .call("POST", "/api/projects", Some(json!({"name": "Contoso", "hourly_rate": 40})))
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let project_id = created["project"]["id"].as_str().unwrap().to_string();
    assert!(created["project"]["color"].as_str().unwrap().starts_with('#'));

    let (status, _) = t
        .call("POST", "/api/projects", Some(json!({"name": "contoso"})))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, state) = t
        .call("PATCH", &format!("/api/sessions/{session_id}"), Some(json!({"project_id": project_id})))
        .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["sessions"][0]["project_name"], "Contoso");
    assert_eq!(state["sessions"][0]["task_number"], 1);
    assert_eq!(state["sessions"][0]["ordinal"], 1);

    // Stop, then start a second session on a new task of the same project.
    let (status, _) = t.call("POST", "/api/sessions/stop", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = t.call("POST", "/api/sessions/stop", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, state) = t
        .call("POST", "/api/sessions/start", Some(json!({"project_id": project_id, "new_task": true})))
        .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["sessions"][0]["task_number"], 2);
    assert_eq!(state["sessions"][0]["ordinal"], 1);
    assert_eq!(state["sessions"][0]["task_session_count"], 1);
    assert_eq!(state["projects"][0]["task_count"], 2);
    assert_eq!(state["projects"][0]["current_task_number"], 2);

    // Untag via explicit null; task 2 is pruned.
    let second_id = state["active_session_id"].as_str().unwrap().to_string();
    let (status, state) = t
        .call("PATCH", &format!("/api/sessions/{second_id}"), Some(json!({"project_id": null})))
        .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert!(state["sessions"][0]["project_id"].is_null());
    assert_eq!(state["projects"][0]["task_count"], 1);

    // Time edit validation.
    let (status, err) = t
        .call(
            "PATCH",
            &format!("/api/sessions/{second_id}"),
            Some(json!({"ended_at": "2000-01-01T00:00:00Z"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("end must be after start"));

    // Project summary & rate change re-prices.
    let (status, state) = t
        .call("PATCH", &format!("/api/projects/{project_id}"), Some(json!({"hourly_rate": 80})))
        .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let (status, summary) = t.call("GET", &format!("/api/projects/{project_id}/summary"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary["hourly_rate"], 80.0);
    assert_eq!(summary["tasks"].as_array().unwrap().len(), 1);

    // Sessions so far are zero-length (created and stopped within the same
    // millisecond); backdate them so the report has something to count.
    let now = chrono::Utc::now();
    let (status, state) = t
        .call(
            "PATCH",
            &format!("/api/sessions/{session_id}"),
            Some(json!({
                "started_at": now - chrono::Duration::hours(3),
                "ended_at": now - chrono::Duration::hours(1),
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let (status, state) = t
        .call(
            "PATCH",
            &format!("/api/sessions/{second_id}"),
            Some(json!({ "started_at": now - chrono::Duration::minutes(30) })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert!(state["sessions"][0]["duration_secs"].as_i64().unwrap() >= 1800);
    assert_eq!(state["sessions"][1]["duration_secs"], 7200);
    assert_eq!(state["sessions"][1]["pay"], 160.0);

    // Report over a window that includes everything.
    let (status, report) = t
        .call("GET", "/api/report?from=2000-01-01T00:00:00Z&to=2100-01-01T00:00:00Z", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    let rows = report["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1]["project_name"], "Untagged");

    let (status, _) = t
        .call("GET", "/api/report?from=2100-01-01T00:00:00Z&to=2000-01-01T00:00:00Z", None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Delete, and confirm persistence hit the file.
    let (status, state) = t.call("DELETE", &format!("/api/sessions/{second_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["sessions"].as_array().unwrap().len(), 1);
    let (status, _) = t.call("DELETE", &format!("/api/sessions/{second_id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let on_disk: Value = serde_json::from_slice(&std::fs::read(&t.path).unwrap()).unwrap();
    assert_eq!(on_disk["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(on_disk["projects"][0]["name"], "Contoso");
}

#[tokio::test]
async fn failed_validation_does_not_persist() {
    let t = test_app().await;
    let (status, _) = t
        .call("POST", "/api/projects", Some(json!({"name": "   "})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!t.path.exists(), "nothing should have been written");
}

#[tokio::test]
async fn unknown_api_route_falls_back_to_frontend_handler() {
    let t = test_app().await;
    let (status, _) = t.call("GET", "/some/spa/route", None).await;
    // Either the built index.html or the "not built" message; never a 404.
    assert!(status == StatusCode::OK || status == StatusCode::SERVICE_UNAVAILABLE);
}
