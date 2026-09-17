//! Pure startup decisions: which mode to run in and where data goes.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// What the window should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Point the window at an already-running Harmony (e.g. the Docker one).
    Attach { url: String },
    /// Run the server in-process with this `HARMONY_STORAGE` spec.
    Embedded { storage_spec: String },
}

/// Resolve the mode from `--url` / `HARMONY_URL` / `HARMONY_STORAGE`, falling
/// back to a local folder under the platform data directory.
pub fn resolve_mode(
    args: impl IntoIterator<Item = String>,
    env: impl Fn(&str) -> Option<String>,
    data_dir: Option<&Path>,
) -> anyhow::Result<Mode> {
    let mut args = args.into_iter();
    let mut url = None;
    while let Some(arg) = args.next() {
        if let Some(v) = arg.strip_prefix("--url=") {
            url = Some(v.to_string());
        } else if arg == "--url" {
            url = Some(args.next().context("--url needs a value")?);
        } else if arg == "--help" || arg == "-h" {
            bail!(
                "usage: harmony-desktop [--url http://host:port]\n\
                 Without --url the server runs in-process using HARMONY_STORAGE\n\
                 (default: file:<local data dir>/Harmony)."
            );
        } else {
            bail!("unknown argument {arg:?}");
        }
    }
    let url = url.or_else(|| env("HARMONY_URL")).map(|u| u.trim().to_string());
    if let Some(url) = url.filter(|u| !u.is_empty()) {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("--url / HARMONY_URL must start with http:// or https:// (got {url:?})");
        }
        return Ok(Mode::Attach { url });
    }

    let spec = env("HARMONY_STORAGE")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let storage_spec = match spec {
        Some(s) => s,
        None => default_storage_spec(data_dir)?,
    };
    Ok(Mode::Embedded { storage_spec })
}

/// `file:<data dir>/Harmony`
pub fn default_storage_spec(data_dir: Option<&Path>) -> anyhow::Result<String> {
    let dir = app_data_dir(data_dir)?;
    Ok(format!("file:{}", dir.display()))
}

/// `<data dir>/Harmony`, where the default store and the log file live.
pub fn app_data_dir(data_dir: Option<&Path>) -> anyhow::Result<PathBuf> {
    let base = data_dir.context(
        "no local data directory on this platform; set HARMONY_STORAGE explicitly",
    )?;
    Ok(base.join("Harmony"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_to_local_folder() {
        let m = resolve_mode(args(&[]), no_env, Some(Path::new("/home/me/.local/share"))).unwrap();
        assert_eq!(
            m,
            Mode::Embedded {
                storage_spec: format!(
                    "file:{}",
                    Path::new("/home/me/.local/share").join("Harmony").display()
                )
            }
        );
    }

    #[test]
    fn env_storage_wins_over_default() {
        let env = |k: &str| (k == "HARMONY_STORAGE").then(|| " sas:https://x/y?sig=1 ".to_string());
        let m = resolve_mode(args(&[]), env, Some(Path::new("/d"))).unwrap();
        assert_eq!(m, Mode::Embedded { storage_spec: "sas:https://x/y?sig=1".into() });
    }

    #[test]
    fn url_flag_and_env_attach() {
        let m = resolve_mode(args(&["--url", "http://localhost:31415"]), no_env, None).unwrap();
        assert_eq!(m, Mode::Attach { url: "http://localhost:31415".into() });
        let m = resolve_mode(args(&["--url=https://h/"]), no_env, None).unwrap();
        assert_eq!(m, Mode::Attach { url: "https://h/".into() });
        let env = |k: &str| (k == "HARMONY_URL").then(|| "http://a:1".to_string());
        let m = resolve_mode(args(&[]), env, None).unwrap();
        assert_eq!(m, Mode::Attach { url: "http://a:1".into() });
    }

    #[test]
    fn rejects_bad_input() {
        assert!(resolve_mode(args(&["--url", "localhost:1"]), no_env, None).is_err());
        assert!(resolve_mode(args(&["--url"]), no_env, None).is_err());
        assert!(resolve_mode(args(&["--bogus"]), no_env, None).is_err());
        assert!(resolve_mode(args(&[]), no_env, None).is_err(), "no data dir, no spec");
    }
}
