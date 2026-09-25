//! Mutations on [`AppData`]. Every function validates and either applies
//! the change or returns a [`DomainError`] leaving the data untouched.

use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;
use uuid::Uuid;

use super::model::{AppData, Project, Session, Settings, Task};
use super::palette::{color_for_index, is_valid_hex_color};

/// Longest payout delay accepted, in days. Generous; it only guards typos.
pub const MAX_PAYOUT_DELAY_DAYS: u32 = 90;

#[derive(Debug, Error, PartialEq)]
pub enum DomainError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, DomainError>;

/// Where a session should be filed.
#[derive(Debug, Clone, PartialEq)]
pub enum Assignment {
    /// Remove the project/task link.
    Untag,
    /// File under a project; the server picks (or creates) the task.
    Project { project_id: Uuid, new_task: bool },
    /// File under an explicit existing task.
    Task(Uuid),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub hourly_rate: Option<f64>,
    pub color: Option<String>,
    pub archived: Option<bool>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SessionPatch {
    pub assignment: Option<Assignment>,
    pub started_at: Option<DateTime<Utc>>,
    /// `Some(None)` clears the end (re-activates), `Some(Some(t))` sets it.
    pub ended_at: Option<Option<DateTime<Utc>>>,
    /// `Some(None)` clears the note.
    pub note: Option<Option<String>>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SettingsPatch {
    /// `Some(None)` clears the cycle start (back to Monday-based weeks).
    pub cycle_start: Option<Option<NaiveDate>>,
    pub payout_delay_days: Option<u32>,
}

fn normalize_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Invalid("project name cannot be empty".into()));
    }
    if trimmed.chars().count() > 80 {
        return Err(DomainError::Invalid("project name is too long (max 80)".into()));
    }
    Ok(trimmed.to_string())
}

fn validate_rate(rate: f64) -> Result<f64> {
    if !rate.is_finite() || rate < 0.0 {
        return Err(DomainError::Invalid(
            "hourly rate must be a non-negative number".into(),
        ));
    }
    Ok(rate)
}

impl AppData {
    // ----- lookups -------------------------------------------------------

    pub fn active_session(&self) -> Option<&Session> {
        self.sessions.iter().find(|s| s.is_active())
    }

    pub fn project(&self, id: Uuid) -> Result<&Project> {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| DomainError::NotFound("project".into()))
    }

    fn project_mut(&mut self, id: Uuid) -> Result<&mut Project> {
        self.projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| DomainError::NotFound("project".into()))
    }

    pub fn task(&self, id: Uuid) -> Result<&Task> {
        self.tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| DomainError::NotFound("task".into()))
    }

    fn task_mut(&mut self, id: Uuid) -> Result<&mut Task> {
        self.tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| DomainError::NotFound("task".into()))
    }

    /// When the task's work stopped: the latest session end. `None` while it
    /// has no ended session.
    pub fn task_last_ended(&self, task_id: Uuid) -> Option<DateTime<Utc>> {
        self.sessions
            .iter()
            .filter(|s| s.task_id == Some(task_id))
            .filter_map(|s| s.ended_at)
            .max()
    }

    /// When the task was turned in, if it was: its last session end.
    pub fn task_completed_at(&self, task: &Task) -> Option<DateTime<Utc>> {
        if task.completed {
            self.task_last_ended(task.id)
        } else {
            None
        }
    }

    fn task_has_active_session(&self, task_id: Uuid) -> bool {
        self.active_session().is_some_and(|s| s.task_id == Some(task_id))
    }

    pub fn session(&self, id: Uuid) -> Result<&Session> {
        self.sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| DomainError::NotFound("session".into()))
    }

    fn session_mut(&mut self, id: Uuid) -> Result<&mut Session> {
        self.sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| DomainError::NotFound("session".into()))
    }

    /// The task new sessions are appended to: the highest-numbered one.
    pub fn current_task(&self, project_id: Uuid) -> Option<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.project_id == project_id)
            .max_by_key(|t| t.number)
    }

    /// Sessions in a task, ordered by start time.
    pub fn task_sessions(&self, task_id: Uuid) -> Vec<&Session> {
        let mut v: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|s| s.task_id == Some(task_id))
            .collect();
        v.sort_by_key(|s| s.started_at);
        v
    }

    /// `(ordinal, total)` of a session within its task, 1-based.
    pub fn session_ordinal(&self, session: &Session) -> Option<(u32, u32)> {
        let task_id = session.task_id?;
        let peers = self.task_sessions(task_id);
        let idx = peers.iter().position(|s| s.id == session.id)?;
        Some((idx as u32 + 1, peers.len() as u32))
    }

    fn name_taken(&self, name: &str, except: Option<Uuid>) -> bool {
        self.projects.iter().any(|p| {
            !p.archived && Some(p.id) != except && p.name.eq_ignore_ascii_case(name)
        })
    }

    // ----- projects ------------------------------------------------------

    pub fn create_project(
        &mut self,
        name: &str,
        hourly_rate: f64,
        now: DateTime<Utc>,
    ) -> Result<&Project> {
        let name = normalize_name(name)?;
        let hourly_rate = validate_rate(hourly_rate)?;
        if self.name_taken(&name, None) {
            return Err(DomainError::Conflict(format!(
                "a project named \"{name}\" already exists"
            )));
        }
        let project = Project {
            id: Uuid::new_v4(),
            name,
            hourly_rate,
            color: color_for_index(self.projects.len()).to_string(),
            created_at: now,
            archived: false,
        };
        self.projects.push(project);
        Ok(self.projects.last().expect("just pushed"))
    }

    pub fn update_project(&mut self, id: Uuid, patch: ProjectPatch) -> Result<&Project> {
        // Validate everything before touching the project.
        let name = patch.name.as_deref().map(normalize_name).transpose()?;
        if let Some(n) = &name
            && self.name_taken(n, Some(id))
        {
            return Err(DomainError::Conflict(format!(
                "a project named \"{n}\" already exists"
            )));
        }
        let rate = patch.hourly_rate.map(validate_rate).transpose()?;
        if let Some(c) = &patch.color
            && !is_valid_hex_color(c)
        {
            return Err(DomainError::Invalid("color must be #rrggbb".into()));
        }
        let project = self.project_mut(id)?;
        if let Some(n) = name {
            project.name = n;
        }
        if let Some(r) = rate {
            project.hourly_rate = r;
        }
        if let Some(c) = patch.color {
            project.color = c.to_ascii_lowercase();
        }
        if let Some(a) = patch.archived {
            project.archived = a;
        }
        Ok(project)
    }

    // ----- tasks ---------------------------------------------------------

    /// Mark a task turned in. Its completion time is its last session end, so
    /// a task with a running (or no ended) session can't be completed yet.
    pub fn complete_task(&mut self, id: Uuid) -> Result<&Task> {
        self.task(id)?;
        if self.task_has_active_session(id) {
            return Err(DomainError::Conflict(
                "stop the running session before marking the task done".into(),
            ));
        }
        if self.task_last_ended(id).is_none() {
            return Err(DomainError::Invalid(
                "a task with no finished session cannot be done".into(),
            ));
        }
        let task = self.task_mut(id)?;
        task.completed = true;
        Ok(task)
    }

    pub fn reopen_task(&mut self, id: Uuid) -> Result<&Task> {
        let task = self.task_mut(id)?;
        task.completed = false;
        Ok(task)
    }

    /// Task to file a session under for `project_id`. Creates the first task
    /// implicitly and a fresh one when `new_task` is requested or the current
    /// task is already turned in. `except` is the session being (re)filed, so
    /// that its own state doesn't count against its old task.
    fn resolve_task(
        &mut self,
        project_id: Uuid,
        new_task: bool,
        except: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<Uuid> {
        self.project(project_id)?;
        let current = self
            .current_task(project_id)
            .map(|t| (t.id, t.number, t.completed));
        let Some((id, number, completed)) = current else {
            return Ok(self.push_task(project_id, 1, now));
        };
        // Re-picking the same project on a session already in the task is a
        // no-op, even when the task is done.
        let already_there = except.is_some_and(|sid| {
            self.session(sid).is_ok_and(|s| s.task_id == Some(id))
        });
        if !new_task && (already_there || !completed) {
            return Ok(id);
        }
        if !completed {
            // Moving on implies the old pickup was turned in, provided it has
            // finished work to date it by and nothing is still running on it.
            let still_running = self
                .active_session()
                .is_some_and(|s| s.task_id == Some(id) && Some(s.id) != except);
            let has_ended = self
                .sessions
                .iter()
                .any(|s| s.task_id == Some(id) && Some(s.id) != except && s.ended_at.is_some());
            if !still_running && has_ended {
                self.task_mut(id)?.completed = true;
            }
        }
        Ok(self.push_task(project_id, number + 1, now))
    }

    fn push_task(&mut self, project_id: Uuid, number: u32, now: DateTime<Utc>) -> Uuid {
        let task = Task {
            id: Uuid::new_v4(),
            project_id,
            number,
            created_at: now,
            completed: false,
        };
        let id = task.id;
        self.tasks.push(task);
        id
    }

    /// Drop tasks no session refers to. Keeps numbering tidy after undo-style
    /// retagging (an accidental "new task" that is immediately retagged away).
    fn prune_empty_tasks(&mut self) {
        let sessions = &self.sessions;
        self.tasks
            .retain(|t| sessions.iter().any(|s| s.task_id == Some(t.id)));
    }

    // ----- sessions ------------------------------------------------------

    pub fn start_session(
        &mut self,
        now: DateTime<Utc>,
        project_id: Option<Uuid>,
        new_task: bool,
    ) -> Result<&Session> {
        if self.active_session().is_some() {
            return Err(DomainError::Conflict("a session is already running".into()));
        }
        let task_id = match project_id {
            Some(pid) => Some(self.resolve_task(pid, new_task, None, now)?),
            None => None,
        };
        self.sessions.push(Session {
            id: Uuid::new_v4(),
            task_id,
            started_at: now,
            ended_at: None,
            note: None,
        });
        Ok(self.sessions.last().expect("just pushed"))
    }

    pub fn stop_session(&mut self, now: DateTime<Utc>) -> Result<&Session> {
        let s = self
            .sessions
            .iter_mut()
            .find(|s| s.is_active())
            .ok_or_else(|| DomainError::NotFound("running session".into()))?;
        s.ended_at = Some(now.max(s.started_at));
        Ok(s)
    }

    pub fn update_session(
        &mut self,
        id: Uuid,
        patch: SessionPatch,
        now: DateTime<Utc>,
    ) -> Result<&Session> {
        let existing = self.session(id)?.clone();

        // Times first, since they can fail without side effects.
        let started_at = patch.started_at.unwrap_or(existing.started_at);
        let ended_at = patch.ended_at.unwrap_or(existing.ended_at);
        if let Some(e) = ended_at {
            if e <= started_at {
                return Err(DomainError::Invalid("end must be after start".into()));
            }
        } else if let Some(active) = self.active_session()
            && active.id != id
        {
            return Err(DomainError::Conflict(
                "another session is already running".into(),
            ));
        }
        if started_at > now + chrono::Duration::minutes(5) {
            return Err(DomainError::Invalid("start cannot be in the future".into()));
        }

        // Assignment may create a task; nothing below can fail after this.
        let task_id = match patch.assignment {
            None => existing.task_id,
            Some(Assignment::Untag) => None,
            Some(Assignment::Task(tid)) => {
                // Filing onto a specific task is deliberate: it reopens a done one.
                self.task_mut(tid)?.completed = false;
                Some(tid)
            }
            Some(Assignment::Project {
                project_id,
                new_task,
            }) => Some(self.resolve_task(project_id, new_task, Some(id), now)?),
        };

        let s = self.session_mut(id)?;
        s.started_at = started_at;
        s.ended_at = ended_at;
        s.task_id = task_id;
        if let Some(note) = patch.note {
            s.note = note
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty());
        }
        // A done task never has work in progress: re-activating reopens it.
        if ended_at.is_none()
            && let Some(tid) = task_id
        {
            self.task_mut(tid)?.completed = false;
        }
        self.prune_empty_tasks();
        self.session(id)
    }

    pub fn delete_session(&mut self, id: Uuid) -> Result<()> {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        if self.sessions.len() == before {
            return Err(DomainError::NotFound("session".into()));
        }
        self.prune_empty_tasks();
        Ok(())
    }

    // ----- settings ------------------------------------------------------

    pub fn update_settings(&mut self, patch: SettingsPatch) -> Result<&Settings> {
        if let Some(d) = patch.payout_delay_days
            && d > MAX_PAYOUT_DELAY_DAYS
        {
            return Err(DomainError::Invalid(format!(
                "payout delay must be at most {MAX_PAYOUT_DELAY_DAYS} days"
            )));
        }
        if let Some(c) = patch.cycle_start {
            self.settings.cycle_start = c;
        }
        if let Some(d) = patch.payout_delay_days {
            self.settings.payout_delay_days = d;
        }
        Ok(&self.settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 16, h, 0, 0).unwrap()
    }

    #[test]
    fn create_project_assigns_colours_in_order() {
        let mut d = AppData::default();
        let a = d.create_project("A", 10.0, t(1)).unwrap().color.clone();
        let b = d.create_project("B", 10.0, t(1)).unwrap().color.clone();
        assert_ne!(a, b);
        assert_eq!(a, color_for_index(0));
    }

    #[test]
    fn create_project_rejects_duplicates_and_blank() {
        let mut d = AppData::default();
        d.create_project("Contoso", 10.0, t(1)).unwrap();
        assert!(matches!(
            d.create_project("  contoso ", 10.0, t(1)),
            Err(DomainError::Conflict(_))
        ));
        assert!(matches!(
            d.create_project("  ", 10.0, t(1)),
            Err(DomainError::Invalid(_))
        ));
        assert!(matches!(
            d.create_project("X", -1.0, t(1)),
            Err(DomainError::Invalid(_))
        ));
    }

    #[test]
    fn archived_name_can_be_reused() {
        let mut d = AppData::default();
        let id = d.create_project("Contoso", 10.0, t(1)).unwrap().id;
        d.update_project(
            id,
            ProjectPatch {
                archived: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        d.create_project("contoso", 12.0, t(2)).unwrap();
        assert_eq!(d.projects.len(), 2);
    }

    #[test]
    fn only_one_active_session() {
        let mut d = AppData::default();
        d.start_session(t(1), None, false).unwrap();
        assert!(matches!(
            d.start_session(t(2), None, false),
            Err(DomainError::Conflict(_))
        ));
        d.stop_session(t(3)).unwrap();
        assert!(matches!(d.stop_session(t(3)), Err(DomainError::NotFound(_))));
        d.start_session(t(4), None, false).unwrap();
        assert_eq!(d.sessions.len(), 2);
    }

    #[test]
    fn first_tag_creates_task_one_then_appends_then_new_task() {
        let mut d = AppData::default();
        let pid = d.create_project("Contoso", 40.0, t(0)).unwrap().id;

        let s1 = d.start_session(t(1), Some(pid), false).unwrap().id;
        d.stop_session(t(2)).unwrap();
        let s2 = d.start_session(t(3), Some(pid), false).unwrap().id;
        d.stop_session(t(4)).unwrap();
        let s3 = d.start_session(t(5), Some(pid), true).unwrap().id;
        d.stop_session(t(6)).unwrap();

        assert_eq!(d.tasks.len(), 2);
        let task1 = d.task(d.session(s1).unwrap().task_id.unwrap()).unwrap();
        let task2 = d.task(d.session(s3).unwrap().task_id.unwrap()).unwrap();
        assert_eq!(task1.number, 1);
        assert_eq!(task2.number, 2);
        assert_eq!(d.session(s2).unwrap().task_id, Some(task1.id));

        assert_eq!(d.session_ordinal(d.session(s1).unwrap()), Some((1, 2)));
        assert_eq!(d.session_ordinal(d.session(s2).unwrap()), Some((2, 2)));
        assert_eq!(d.session_ordinal(d.session(s3).unwrap()), Some((1, 1)));
        assert_eq!(d.current_task(pid).unwrap().number, 2);
    }

    #[test]
    fn retag_prunes_empty_task_and_reuses_number() {
        let mut d = AppData::default();
        let pid = d.create_project("Contoso", 40.0, t(0)).unwrap().id;
        d.start_session(t(1), Some(pid), false).unwrap();
        d.stop_session(t(2)).unwrap();
        let s2 = d.start_session(t(3), Some(pid), true).unwrap().id; // task 2 by mistake
        d.stop_session(t(4)).unwrap();
        assert_eq!(d.tasks.len(), 2);

        // Move it back onto task 1 explicitly.
        let task1 = d.tasks.iter().find(|t| t.number == 1).unwrap().id;
        d.update_session(
            s2,
            SessionPatch {
                assignment: Some(Assignment::Task(task1)),
                ..Default::default()
            },
            t(5),
        )
        .unwrap();
        assert_eq!(d.tasks.len(), 1, "empty task 2 pruned");

        // A subsequent "new task" gets number 2 again.
        d.start_session(t(6), Some(pid), true).unwrap();
        assert_eq!(d.current_task(pid).unwrap().number, 2);
    }

    #[test]
    fn untag_and_delete() {
        let mut d = AppData::default();
        let pid = d.create_project("Contoso", 40.0, t(0)).unwrap().id;
        let sid = d.start_session(t(1), Some(pid), false).unwrap().id;
        d.stop_session(t(2)).unwrap();
        d.update_session(
            sid,
            SessionPatch {
                assignment: Some(Assignment::Untag),
                ..Default::default()
            },
            t(3),
        )
        .unwrap();
        assert!(d.tasks.is_empty());
        assert_eq!(d.session(sid).unwrap().task_id, None);
        d.delete_session(sid).unwrap();
        assert!(matches!(d.delete_session(sid), Err(DomainError::NotFound(_))));
    }

    #[test]
    fn time_edits_are_validated() {
        let mut d = AppData::default();
        let sid = d.start_session(t(1), None, false).unwrap().id;
        d.stop_session(t(2)).unwrap();
        let bad = SessionPatch {
            ended_at: Some(Some(t(0))),
            ..Default::default()
        };
        assert!(matches!(
            d.update_session(sid, bad, t(5)),
            Err(DomainError::Invalid(_))
        ));

        // Re-activating is allowed when nothing else runs...
        d.update_session(
            sid,
            SessionPatch {
                ended_at: Some(None),
                ..Default::default()
            },
            t(5),
        )
        .unwrap();
        assert!(d.session(sid).unwrap().is_active());
        d.stop_session(t(3)).unwrap();

        // ...but not when another session is running.
        d.start_session(t(4), None, false).unwrap();
        assert!(matches!(
            d.update_session(
                sid,
                SessionPatch {
                    ended_at: Some(None),
                    ..Default::default()
                },
                t(5)
            ),
            Err(DomainError::Conflict(_))
        ));
    }

    #[test]
    fn stop_never_produces_negative_duration() {
        let mut d = AppData::default();
        d.start_session(t(5), None, false).unwrap();
        let s = d.stop_session(t(1)).unwrap();
        assert_eq!(s.duration_secs(t(9)), 0);
    }

    fn project_with_task(d: &mut AppData) -> (Uuid, Uuid) {
        let pid = d.create_project("Contoso", 40.0, t(0)).unwrap().id;
        let sid = d.start_session(t(1), Some(pid), false).unwrap().id;
        let tid = d.session(sid).unwrap().task_id.unwrap();
        (pid, tid)
    }

    #[test]
    fn complete_task_needs_finished_work_and_dates_it_by_the_last_end() {
        let mut d = AppData::default();
        let (pid, tid) = project_with_task(&mut d);
        assert!(matches!(d.complete_task(tid), Err(DomainError::Conflict(_))), "running");
        d.stop_session(t(2)).unwrap();
        d.start_session(t(3), Some(pid), false).unwrap();
        d.stop_session(t(4)).unwrap();

        assert_eq!(d.task_completed_at(d.task(tid).unwrap()), None);
        d.complete_task(tid).unwrap();
        assert!(d.task(tid).unwrap().completed);
        assert_eq!(d.task_completed_at(d.task(tid).unwrap()), Some(t(4)));
        d.complete_task(tid).unwrap(); // idempotent

        d.reopen_task(tid).unwrap();
        assert!(!d.task(tid).unwrap().completed);
        assert!(matches!(d.complete_task(Uuid::new_v4()), Err(DomainError::NotFound(_))));
    }

    #[test]
    fn new_task_auto_completes_the_previous_pickup() {
        let mut d = AppData::default();
        let (pid, t1) = project_with_task(&mut d);
        d.stop_session(t(2)).unwrap();
        d.start_session(t(3), Some(pid), true).unwrap();
        assert!(d.task(t1).unwrap().completed);
        assert_eq!(d.task_completed_at(d.task(t1).unwrap()), Some(t(2)));
        assert!(!d.current_task(pid).unwrap().completed);
    }

    #[test]
    fn new_task_leaves_the_previous_one_open_while_it_is_running() {
        let mut d = AppData::default();
        let (pid, t1) = project_with_task(&mut d);
        d.stop_session(t(2)).unwrap();
        // An ended untagged session gets retagged as a new task while task 1 runs.
        d.start_session(t(3), Some(pid), false).unwrap(); // running on task 1
        let extra = Session {
            id: Uuid::new_v4(),
            task_id: None,
            started_at: t(4),
            ended_at: Some(t(5)),
            note: None,
        };
        // Insert an ended untagged session by hand (start would conflict).
        d.sessions.push(extra.clone());
        d.update_session(
            extra.id,
            SessionPatch {
                assignment: Some(Assignment::Project { project_id: pid, new_task: true }),
                ..Default::default()
            },
            t(6),
        )
        .unwrap();
        assert!(!d.task(t1).unwrap().completed, "still running");
        assert_eq!(d.current_task(pid).unwrap().number, 2);
    }

    #[test]
    fn moving_the_running_session_to_a_new_task_completes_its_old_task() {
        let mut d = AppData::default();
        let (pid, t1) = project_with_task(&mut d);
        d.stop_session(t(2)).unwrap();
        let sid = d.start_session(t(3), Some(pid), false).unwrap().id;
        d.update_session(
            sid,
            SessionPatch {
                assignment: Some(Assignment::Project { project_id: pid, new_task: true }),
                ..Default::default()
            },
            t(4),
        )
        .unwrap();
        // The moved session no longer counts against task 1, which has t(1)-t(2).
        assert!(d.task(t1).unwrap().completed);
        assert_eq!(d.session(sid).unwrap().task_id, Some(d.current_task(pid).unwrap().id));
        assert!(!d.current_task(pid).unwrap().completed);
    }

    #[test]
    fn start_after_done_opens_the_next_task() {
        let mut d = AppData::default();
        let (pid, t1) = project_with_task(&mut d);
        d.stop_session(t(2)).unwrap();
        d.complete_task(t1).unwrap();
        let s = d.start_session(t(3), Some(pid), false).unwrap();
        let t2 = s.task_id.unwrap();
        assert_ne!(t2, t1);
        assert_eq!(d.task(t2).unwrap().number, 2);
        assert!(d.task(t1).unwrap().completed, "untouched");
    }

    #[test]
    fn repicking_the_same_project_on_a_done_task_is_a_no_op() {
        let mut d = AppData::default();
        let (pid, t1) = project_with_task(&mut d);
        d.stop_session(t(2)).unwrap();
        d.complete_task(t1).unwrap();
        let sid = d.sessions[0].id;
        d.update_session(
            sid,
            SessionPatch {
                assignment: Some(Assignment::Project { project_id: pid, new_task: false }),
                ..Default::default()
            },
            t(3),
        )
        .unwrap();
        assert_eq!(d.session(sid).unwrap().task_id, Some(t1));
        assert_eq!(d.tasks.len(), 1);
        assert!(d.task(t1).unwrap().completed);
    }

    #[test]
    fn explicit_task_assignment_and_reactivation_reopen_a_done_task() {
        let mut d = AppData::default();
        let (pid, t1) = project_with_task(&mut d);
        d.stop_session(t(2)).unwrap();
        let s2 = d.start_session(t(3), Some(pid), true).unwrap().id; // completes task 1
        d.stop_session(t(4)).unwrap();
        assert!(d.task(t1).unwrap().completed);

        d.update_session(
            s2,
            SessionPatch { assignment: Some(Assignment::Task(t1)), ..Default::default() },
            t(5),
        )
        .unwrap();
        assert!(!d.task(t1).unwrap().completed, "explicit choice reopens");
        assert_eq!(d.tasks.len(), 1, "empty task 2 pruned");

        d.complete_task(t1).unwrap();
        d.update_session(s2, SessionPatch { ended_at: Some(None), ..Default::default() }, t(6)).unwrap();
        assert!(!d.task(t1).unwrap().completed, "work in progress reopens");
    }

    #[test]
    fn settings_patch_validates_and_clears() {
        let mut d = AppData::default();
        let date = NaiveDate::from_ymd_opt(2026, 9, 24);
        d.update_settings(SettingsPatch { cycle_start: Some(date), payout_delay_days: Some(10) })
            .unwrap();
        assert_eq!(d.settings.cycle_start, date);
        assert_eq!(d.settings.payout_delay_days, 10);
        assert!(matches!(
            d.update_settings(SettingsPatch { payout_delay_days: Some(91), ..Default::default() }),
            Err(DomainError::Invalid(_))
        ));
        assert_eq!(d.settings.payout_delay_days, 10, "rejected patch leaves data alone");
        d.update_settings(SettingsPatch { cycle_start: Some(None), ..Default::default() }).unwrap();
        assert_eq!(d.settings.cycle_start, None);
    }
}
