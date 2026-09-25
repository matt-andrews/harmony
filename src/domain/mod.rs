//! Pure, storage-agnostic business logic. No I/O in this module tree.

pub mod model;
pub mod ops;
pub mod palette;
pub mod views;

pub use model::{AppData, Project, Session, Settings, Task};
pub use ops::{Assignment, DomainError, ProjectPatch, SessionPatch, SettingsPatch};
