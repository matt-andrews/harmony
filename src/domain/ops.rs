//! Mutations on [`AppData`]. Every function validates and either applies
//! the change or returns a [`DomainError`] leaving the data untouched.

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use super::model::{AppData, Project, Session, Task};
use super::palette::{color_for_index, is_valid_hex_color};

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

    /// Task to file a session under for `project_id`. Creates the first task
    /// implicitly and a fresh one when `new_task` is requested.
    fn resolve_task(
        &mut self,
        project_id: Uuid,
        new_task: bool,
        now: DateTime<Utc>,
    ) -> Result<Uuid> {
        self.project(project_id)?;
        let current = self.current_task(project_id).map(|t| (t.id, t.number));
        match current {
            Some((id, _)) if !new_task => Ok(id),
            other => {
                let number = other.map(|(_, n)| n + 1).unwrap_or(1);
                let task = Task {
                    id: Uuid::new_v4(),
                    project_id,
                    number,
                    created_at: now,
                };
                let id = task.id;
                self.tasks.push(task);
                Ok(id)
            }
        }
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
            Some(pid) => Some(self.resolve_task(pid, new_task, now)?),
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
                self.task(tid)?;
                Some(tid)
            }
            Some(Assignment::Project {
                project_id,
                new_task,
            }) => Some(self.resolve_task(project_id, new_task, now)?),
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
}
