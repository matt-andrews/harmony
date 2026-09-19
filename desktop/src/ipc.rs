//! Messages the frontend posts through `window.ipc.postMessage`.
//!
//! The page decides *when* to be compact; the window only reshapes itself.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    /// Collapse into the floating always-on-top strip.
    Compact,
    /// Back to the full window.
    Expand,
    /// The pointer went down on the compact strip: start a native window drag.
    Drag,
    SetCompactOnStart { value: bool },
}

/// Anything that isn't a known message is ignored; the page may be newer or
/// older than the window (`--url` attaches to any running instance).
pub fn parse(body: &str) -> Option<IpcMessage> {
    serde_json::from_str(body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_message() {
        assert_eq!(parse(r#"{"type":"compact"}"#), Some(IpcMessage::Compact));
        assert_eq!(parse(r#"{"type":"expand"}"#), Some(IpcMessage::Expand));
        assert_eq!(parse(r#"{"type":"drag"}"#), Some(IpcMessage::Drag));
        assert_eq!(
            parse(r#"{"type":"set_compact_on_start","value":true}"#),
            Some(IpcMessage::SetCompactOnStart { value: true })
        );
    }

    #[test]
    fn ignores_everything_else() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("compact"), None);
        assert_eq!(parse(r#"{"type":"self_destruct"}"#), None);
        assert_eq!(parse(r#"{"type":"set_compact_on_start"}"#), None);
    }
}
