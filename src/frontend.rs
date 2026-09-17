//! Serves the built Svelte app from `web/dist`, embedded into the binary in
//! release builds and read from disk in debug builds.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if !path.is_empty()
        && let Some(file) = Assets::get(path)
    {
        return file_response(path, file.data.into_owned());
    }
    match Assets::get("index.html") {
        Some(index) => file_response("index.html", index.data.into_owned()),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Frontend not built. Run `npm run build` in ./web and restart.",
        )
            .into_response(),
    }
}

fn file_response(path: &str, body: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path.starts_with("assets/") {
        // Vite fingerprints everything under assets/.
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        body,
    )
        .into_response()
}
