//! Read models derived from [`AppData`]: what the API hands to the UI.
//! The frontend never computes totals itself.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::model::{AppData, Project, Session, Settings, Task, pay_for, round_cents};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    #[serde(flatten)]
    pub project: Project,
    pub total_secs: i64,
    pub total_pay: f64,
    pub task_count: u32,
    pub current_task_number: Option<u32>,
    /// Time logged so far on the current task, running session included.
    pub current_task_total_secs: Option<i64>,
    /// Set when the current task is turned in: the next Start opens a new one.
    pub current_task_completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub duration_secs: i64,
    pub pay: f64,
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub color: Option<String>,
    pub task_id: Option<Uuid>,
    pub task_number: Option<u32>,
    /// 1-based position within the task; 1 marks the first session of a pickup.
    pub ordinal: Option<u32>,
    pub task_session_count: Option<u32>,
    /// Sum over every session of this session's task, itself included.
    pub task_total_secs: Option<i64>,
    /// When the task was turned in (its last session end), if it was.
    pub task_completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateView {
    pub server_time: DateTime<Utc>,
    pub app_version: &'static str,
    pub active_session_id: Option<Uuid>,
    /// Project of the most recent tagged session: what Start resumes by default.
    pub resume_project_id: Option<Uuid>,
    pub projects: Vec<ProjectView>,
    /// Newest first.
    pub sessions: Vec<SessionView>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskSummary {
    pub id: Uuid,
    pub number: u32,
    pub created_at: DateTime<Utc>,
    pub session_count: u32,
    pub total_secs: i64,
    pub total_pay: f64,
    pub first_started: Option<DateTime<Utc>>,
    pub last_ended: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub project: ProjectView,
    /// Newest task first.
    pub tasks: Vec<TaskSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportRow {
    pub project_id: Option<Uuid>,
    pub project_name: String,
    pub color: Option<String>,
    pub session_count: u32,
    pub total_secs: i64,
    pub total_pay: f64,
}

/// A turned-in task, priced whole: what one payout line is.
#[derive(Debug, Clone, Serialize)]
pub struct PayoutRow {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub color: String,
    pub task_number: u32,
    pub completed_at: DateTime<Utc>,
    pub total_secs: i64,
    pub total_pay: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub rows: Vec<ReportRow>,
    pub total_secs: i64,
    pub total_pay: f64,
    /// Tasks completed inside the requested completion window: withdrawable
    /// within `[from, to)`. Empty when no window was given.
    pub payout_rows: Vec<PayoutRow>,
    pub payout_total_pay: f64,
    /// Tasks completed after the window but before `to`: they pay out next period.
    pub carried_rows: Vec<PayoutRow>,
    pub carried_total_pay: f64,
}

/// Seconds of `session` that fall inside `[from, to)`.
fn overlap_secs(
    session: &Session,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    now: DateTime<Utc>,
) -> i64 {
    let start = session.started_at.max(from);
    let end = session.end_or(now).min(to);
    (end - start).num_seconds().max(0)
}

impl AppData {
    fn project_of_session(&self, s: &Session) -> Option<&Project> {
        let task = self.tasks.iter().find(|t| Some(t.id) == s.task_id)?;
        self.projects.iter().find(|p| p.id == task.project_id)
    }

    fn task_total_secs(&self, task_id: Uuid, now: DateTime<Utc>) -> i64 {
        self.task_sessions(task_id)
            .iter()
            .map(|s| s.duration_secs(now))
            .sum()
    }

    pub fn project_view(&self, project: &Project, now: DateTime<Utc>) -> ProjectView {
        let task_ids: Vec<Uuid> = self
            .tasks
            .iter()
            .filter(|t| t.project_id == project.id)
            .map(|t| t.id)
            .collect();
        let total_secs: i64 = self
            .sessions
            .iter()
            .filter(|s| s.task_id.is_some_and(|tid| task_ids.contains(&tid)))
            .map(|s| s.duration_secs(now))
            .sum();
        let current_task = self.current_task(project.id);
        ProjectView {
            project: project.clone(),
            total_secs,
            total_pay: pay_for(total_secs, project.hourly_rate),
            task_count: task_ids.len() as u32,
            current_task_number: current_task.map(|t| t.number),
            current_task_total_secs: current_task.map(|t| self.task_total_secs(t.id, now)),
            current_task_completed_at: current_task.and_then(|t| self.task_completed_at(t)),
        }
    }

    pub fn session_view(&self, s: &Session, now: DateTime<Utc>) -> SessionView {
        let project = self.project_of_session(s);
        let task = self.tasks.iter().find(|t| Some(t.id) == s.task_id);
        let ordinal = self.session_ordinal(s);
        let secs = s.duration_secs(now);
        SessionView {
            id: s.id,
            started_at: s.started_at,
            ended_at: s.ended_at,
            note: s.note.clone(),
            duration_secs: secs,
            pay: project
                .map(|p| pay_for(secs, p.hourly_rate))
                .unwrap_or(0.0),
            project_id: project.map(|p| p.id),
            project_name: project.map(|p| p.name.clone()),
            color: project.map(|p| p.color.clone()),
            task_id: s.task_id,
            task_number: task.map(|t| t.number),
            ordinal: ordinal.map(|(o, _)| o),
            task_session_count: ordinal.map(|(_, n)| n),
            task_total_secs: task.map(|t| self.task_total_secs(t.id, now)),
            task_completed_at: task.and_then(|t| self.task_completed_at(t)),
        }
    }

    pub fn state_view(&self, now: DateTime<Utc>) -> StateView {
        let mut sessions: Vec<SessionView> = self
            .sessions
            .iter()
            .map(|s| self.session_view(s, now))
            .collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));
        let mut projects: Vec<ProjectView> = self
            .projects
            .iter()
            .map(|p| self.project_view(p, now))
            .collect();
        projects.sort_by_key(|p| p.project.name.to_lowercase());
        let resume_project_id = sessions
            .iter()
            .filter_map(|s| s.project_id)
            .find(|id| projects.iter().any(|p| p.project.id == *id && !p.project.archived));
        StateView {
            server_time: now,
            app_version: crate::VERSION,
            active_session_id: self.active_session().map(|s| s.id),
            resume_project_id,
            projects,
            sessions,
            settings: self.settings.clone(),
        }
    }

    pub fn project_summary(
        &self,
        project_id: Uuid,
        now: DateTime<Utc>,
    ) -> super::ops::Result<ProjectSummary> {
        let project = self.project(project_id)?;
        let mut tasks: Vec<TaskSummary> = self
            .tasks
            .iter()
            .filter(|t| t.project_id == project_id)
            .map(|t| {
                let sessions = self.task_sessions(t.id);
                let total_secs = self.task_total_secs(t.id, now);
                TaskSummary {
                    id: t.id,
                    number: t.number,
                    created_at: t.created_at,
                    session_count: sessions.len() as u32,
                    total_secs,
                    total_pay: pay_for(total_secs, project.hourly_rate),
                    first_started: sessions.first().map(|s| s.started_at),
                    last_ended: self.task_last_ended(t.id),
                    completed_at: self.task_completed_at(t),
                }
            })
            .collect();
        tasks.sort_by_key(|t| std::cmp::Reverse(t.number));
        Ok(ProjectSummary {
            project: self.project_view(project, now),
            tasks,
        })
    }

    fn payout_row(&self, task: &Task, completed_at: DateTime<Utc>, now: DateTime<Utc>) -> Option<PayoutRow> {
        let project = self.project(task.project_id).ok()?;
        let total_secs = self.task_total_secs(task.id, now);
        Some(PayoutRow {
            task_id: task.id,
            project_id: project.id,
            project_name: project.name.clone(),
            color: project.color.clone(),
            task_number: task.number,
            completed_at,
            total_secs,
            total_pay: pay_for(total_secs, project.hourly_rate),
        })
    }

    /// Turned-in tasks whose completion falls in `[from, to)`, oldest first.
    fn payouts(&self, from: DateTime<Utc>, to: DateTime<Utc>, now: DateTime<Utc>) -> Vec<PayoutRow> {
        let mut rows: Vec<PayoutRow> = self
            .tasks
            .iter()
            .filter_map(|t| {
                let done = self.task_completed_at(t)?;
                if done < from || done >= to {
                    return None;
                }
                self.payout_row(t, done, now)
            })
            .collect();
        rows.sort_by(|a, b| {
            a.completed_at
                .cmp(&b.completed_at)
                .then_with(|| a.project_name.to_lowercase().cmp(&b.project_name.to_lowercase()))
        });
        rows
    }

    /// Totals for sessions overlapping `[from, to)`, clipped to the window.
    /// With `completed = Some((cf, ct))`, also the payout section: tasks turned
    /// in within `[cf, ct)` are payable in this period, and those turned in
    /// within `[ct, to)` are carried to the next one.
    pub fn report(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        completed: Option<(DateTime<Utc>, DateTime<Utc>)>,
        now: DateTime<Utc>,
    ) -> Report {
        // Accumulate per project id (None = untagged): (id, count, secs).
        let mut acc: Vec<(Option<Uuid>, u32, i64)> = Vec::new();
        for s in &self.sessions {
            let secs = overlap_secs(s, from, to, now);
            if secs <= 0 {
                continue;
            }
            let pid = self.project_of_session(s).map(|p| p.id);
            match acc.iter_mut().find(|(id, _, _)| *id == pid) {
                Some(entry) => {
                    entry.1 += 1;
                    entry.2 += secs;
                }
                None => acc.push((pid, 1, secs)),
            }
        }
        let mut rows: Vec<ReportRow> = acc
            .into_iter()
            .map(|(pid, count, secs)| {
                let project = pid.and_then(|id| self.project(id).ok());
                ReportRow {
                    project_id: pid,
                    project_name: project
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Untagged".into()),
                    color: project.map(|p| p.color.clone()),
                    session_count: count,
                    total_secs: secs,
                    total_pay: project
                        .map(|p| pay_for(secs, p.hourly_rate))
                        .unwrap_or(0.0),
                }
            })
            .collect();
        // Most time first; untagged last.
        rows.sort_by(|a, b| {
            a.project_id
                .is_none()
                .cmp(&b.project_id.is_none())
                .then(b.total_secs.cmp(&a.total_secs))
        });
        let total_secs = rows.iter().map(|r| r.total_secs).sum();
        let total_pay = round_cents(rows.iter().map(|r| r.total_pay).sum());
        let (payout_rows, carried_rows) = match completed {
            Some((cf, ct)) => (self.payouts(cf, ct, now), self.payouts(ct.max(from), to, now)),
            None => (Vec::new(), Vec::new()),
        };
        let sum_pay = |rows: &[PayoutRow]| round_cents(rows.iter().map(|r| r.total_pay).sum());
        Report {
            from,
            to,
            rows,
            total_secs,
            total_pay,
            payout_total_pay: sum_pay(&payout_rows),
            payout_rows,
            carried_total_pay: sum_pay(&carried_rows),
            carried_rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 16, h, 0, 0).unwrap()
    }

    fn fixture() -> (AppData, Uuid, Uuid) {
        let mut d = AppData::default();
        let a = d.create_project("Alpha", 40.0, t(0)).unwrap().id;
        let b = d.create_project("Beta", 100.0, t(0)).unwrap().id;
        d.start_session(t(1), Some(a), false).unwrap(); // 1-3 alpha (2h)
        d.stop_session(t(3)).unwrap();
        d.start_session(t(4), Some(b), false).unwrap(); // 4-5 beta (1h)
        d.stop_session(t(5)).unwrap();
        d.start_session(t(6), None, false).unwrap(); // 6-7 untagged
        d.stop_session(t(7)).unwrap();
        d.start_session(t(8), Some(a), true).unwrap(); // 8-? alpha task 2, running
        (d, a, b)
    }

    #[test]
    fn state_view_is_newest_first_with_ordinals_and_live_duration() {
        let (d, a, _) = fixture();
        let now = t(9);
        let v = d.state_view(now);
        assert_eq!(v.sessions.len(), 4);
        assert_eq!(v.sessions[0].started_at, t(8));
        assert_eq!(v.sessions[0].duration_secs, 3600);
        assert_eq!(v.sessions[0].task_number, Some(2));
        assert_eq!(v.sessions[0].ordinal, Some(1));
        assert_eq!(v.sessions[0].pay, 40.0);
        assert_eq!(v.active_session_id, Some(v.sessions[0].id));
        assert!(v.sessions[1].project_id.is_none());
        let alpha = v.projects.iter().find(|p| p.project.id == a).unwrap();
        assert_eq!(alpha.total_secs, 3 * 3600);
        assert_eq!(alpha.total_pay, 120.0);
        assert_eq!(alpha.task_count, 2);
        assert_eq!(alpha.current_task_number, Some(2));
        assert_eq!(alpha.current_task_total_secs, Some(3600));
        assert_eq!(v.sessions[0].task_total_secs, Some(3600));
        assert_eq!(v.sessions[1].task_total_secs, None);
    }

    #[test]
    fn task_total_sums_every_session_of_the_task() {
        let (mut d, a, _) = fixture();
        d.stop_session(t(9)).unwrap(); // alpha task 2: 8-9
        d.start_session(t(10), Some(a), false).unwrap(); // alpha task 2 again, running
        let v = d.state_view(t(12));
        // Both sessions of task 2 report 1h + 2h; task 1 is unaffected.
        assert_eq!(v.sessions[0].task_total_secs, Some(3 * 3600));
        assert_eq!(v.sessions[1].task_total_secs, Some(3 * 3600));
        assert_eq!(v.sessions.last().unwrap().task_total_secs, Some(2 * 3600));
        let alpha = v.projects.iter().find(|p| p.project.id == a).unwrap();
        assert_eq!(alpha.current_task_total_secs, Some(3 * 3600));
    }

    #[test]
    fn resume_project_is_latest_tagged_and_not_archived() {
        use crate::domain::ops::ProjectPatch;

        assert_eq!(AppData::default().state_view(t(0)).resume_project_id, None);

        let (mut d, a, b) = fixture();
        assert_eq!(d.state_view(t(9)).resume_project_id, Some(a));

        // An untagged session on top doesn't displace it.
        d.stop_session(t(9)).unwrap();
        d.start_session(t(10), None, false).unwrap();
        d.stop_session(t(11)).unwrap();
        assert_eq!(d.state_view(t(12)).resume_project_id, Some(a));

        // Archiving falls back to the next most recent project.
        let archive = ProjectPatch { archived: Some(true), ..Default::default() };
        d.update_project(a, archive).unwrap();
        assert_eq!(d.state_view(t(12)).resume_project_id, Some(b));
    }

    #[test]
    fn report_clips_to_window_and_sorts() {
        let (d, a, b) = fixture();
        let now = t(9);
        // Window 2:00-4:30 catches 1h of alpha (2-3) and 30m of beta (4-4:30).
        let r = d.report(t(2), t(4) + chrono::Duration::minutes(30), None, now);
        assert_eq!(r.rows.len(), 2);
        assert_eq!(r.rows[0].project_id, Some(a));
        assert_eq!(r.rows[0].total_secs, 3600);
        assert_eq!(r.rows[0].total_pay, 40.0);
        assert_eq!(r.rows[1].project_id, Some(b));
        assert_eq!(r.rows[1].total_secs, 1800);
        assert_eq!(r.rows[1].total_pay, 50.0);
        assert_eq!(r.total_secs, 5400);
        assert_eq!(r.total_pay, 90.0);

        // Full day: untagged row is last, running session counts to `now`.
        let r = d.report(t(0), t(23), None, now);
        assert_eq!(r.rows.last().unwrap().project_name, "Untagged");
        let alpha = r.rows.iter().find(|r| r.project_id == Some(a)).unwrap();
        assert_eq!(alpha.total_secs, 3 * 3600);
        assert_eq!(alpha.session_count, 2);
        assert!(r.payout_rows.is_empty() && r.carried_rows.is_empty());
    }

    #[test]
    fn report_payouts_follow_the_completion_window() {
        let (mut d, a, b) = fixture();
        d.stop_session(t(9)).unwrap(); // alpha task 2: 8-9
        // Alpha task 1 (1-3, 2h @ $40) was auto-completed at t(3) when task 2
        // started; turn in beta's task (4-5, 1h @ $100) and alpha task 2 (1h).
        let beta_task = d.current_task(b).unwrap().id;
        d.complete_task(beta_task).unwrap();
        let alpha2 = d.current_task(a).unwrap().id;
        d.complete_task(alpha2).unwrap();
        let now = t(12);

        // Period 6-12 with completion window 2-6: alpha #1 (done 3) and beta
        // (done 5) pay out; alpha #2 (done 9) is carried.
        let r = d.report(t(6), t(12), Some((t(2), t(6))), now);
        let ids: Vec<_> = r.payout_rows.iter().map(|p| (p.task_number, p.completed_at)).collect();
        assert_eq!(ids, vec![(1, t(3)), (1, t(5))], "oldest completion first");
        assert_eq!(r.payout_rows[0].project_id, a);
        assert_eq!(r.payout_rows[0].total_secs, 7200);
        assert_eq!(r.payout_rows[0].total_pay, 80.0);
        assert_eq!(r.payout_rows[1].total_pay, 100.0);
        assert_eq!(r.payout_total_pay, 180.0);
        assert_eq!(r.carried_rows.len(), 1);
        assert_eq!(r.carried_rows[0].task_id, alpha2);
        assert_eq!(r.carried_total_pay, 40.0);

        // A rate change re-prices a past payout, like everything else.
        use crate::domain::ops::ProjectPatch;
        d.update_project(a, ProjectPatch { hourly_rate: Some(50.0), ..Default::default() }).unwrap();
        let r = d.report(t(6), t(12), Some((t(2), t(6))), now);
        assert_eq!(r.payout_rows[0].total_pay, 100.0);

        // Reopening drops the task from every payout.
        d.reopen_task(beta_task).unwrap();
        let r = d.report(t(6), t(12), Some((t(2), t(6))), now);
        assert_eq!(r.payout_rows.len(), 1);
    }

    #[test]
    fn views_expose_completion_and_settings() {
        let (mut d, a, _) = fixture();
        d.settings.payout_delay_days = 3;
        let v = d.state_view(t(9));
        assert_eq!(v.app_version, crate::VERSION);
        assert_eq!(v.settings.payout_delay_days, 3);
        // Alpha task 1 was auto-completed when task 2 started (last end t(3)).
        let old = v.sessions.iter().find(|s| s.task_number == Some(1) && s.project_id == Some(a)).unwrap();
        assert_eq!(old.task_completed_at, Some(t(3)));
        assert_eq!(v.sessions[0].task_completed_at, None, "running task 2 is open");
        let alpha = v.projects.iter().find(|p| p.project.id == a).unwrap();
        assert_eq!(alpha.current_task_completed_at, None);

        d.stop_session(t(9)).unwrap();
        let t2 = d.current_task(a).unwrap().id;
        d.complete_task(t2).unwrap();
        let v = d.state_view(t(10));
        let alpha = v.projects.iter().find(|p| p.project.id == a).unwrap();
        assert_eq!(alpha.current_task_completed_at, Some(t(9)));
        let s = d.project_summary(a, t(10)).unwrap();
        assert_eq!(s.tasks[0].completed_at, Some(t(9)));
        assert_eq!(s.tasks[1].completed_at, Some(t(3)));
    }

    #[test]
    fn project_summary_lists_tasks_newest_first() {
        let (d, a, _) = fixture();
        let s = d.project_summary(a, t(9)).unwrap();
        assert_eq!(s.tasks.len(), 2);
        assert_eq!(s.tasks[0].number, 2);
        assert_eq!(s.tasks[0].last_ended, None);
        assert_eq!(s.tasks[1].number, 1);
        assert_eq!(s.tasks[1].total_secs, 7200);
        assert_eq!(s.tasks[1].last_ended, Some(t(3)));
        assert!(d.project_summary(Uuid::new_v4(), t(9)).is_err());
    }
}
