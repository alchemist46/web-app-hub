//! Self-invoked `launch` subcommand used by the generated desktop files of
//! isolated Chromium web apps to give them native-feeling window memory.
//!
//! Chromium `--app=URL` windows save their geometry to the profile's
//! `Preferences` (`browser.app_window_placement`) on close, but never restore it
//! on the next launch. The desktop file therefore routes the browser command
//! through `web-app-hub launch --profile <dir> -- <browser> [args...]`. This
//! reads the saved placement for the (single) app in that isolated profile and
//! appends `--window-size` / `--window-position` (or `--start-maximized`) before
//! `exec`ing the real browser, so the window reopens where the user left it.
//!
//! Only isolated Chromium apps use this path; Firefox-based apps already remember
//! their geometry per-profile, and non-isolated apps share the user's real
//! browser profile and are left untouched.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value};

const SUBCOMMAND: &str = "launch";

/// Intercept a `launch` invocation before any GTK/i18n setup. Returns `false`
/// for a normal GUI start so `main` proceeds. On a launch invocation this never
/// returns: it either `exec`s the browser (replacing this process) or exits.
pub fn maybe_run() -> bool {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some(SUBCOMMAND) {
        return false;
    }

    run(args.collect())
}

fn run(args: Vec<String>) -> ! {
    let (profile, mut browser_cmd) = parse_args(args);

    if browser_cmd.is_empty() {
        eprintln!("web-app-hub {SUBCOMMAND}: no browser command after '--'");
        std::process::exit(2);
    }

    if let Some(profile) = &profile {
        clear_stale_singleton_lock(profile);
    }

    if let Some(profile) = &profile
        && let Some(flags) = geometry_flags(profile, &browser_cmd)
    {
        browser_cmd.extend(flags);
    }

    // Replace this process with the browser so the desktop launcher tracks the
    // browser's lifetime, not ours.
    let error = Command::new(&browser_cmd[0]).args(&browser_cmd[1..]).exec();

    eprintln!(
        "web-app-hub {SUBCOMMAND}: failed to exec '{}': {error}",
        browser_cmd[0]
    );
    std::process::exit(126);
}

/// Split `--profile <dir> -- <browser> [args...]` into the profile path and the
/// browser command line. Unknown flags before `--` are ignored.
fn parse_args(args: Vec<String>) -> (Option<PathBuf>, Vec<String>) {
    let mut profile = None;
    let mut browser_cmd = Vec::new();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--profile" => profile = iter.next().map(PathBuf::from),
            "--" => {
                browser_cmd.extend(iter.by_ref());
                break;
            }
            _ => {}
        }
    }

    (profile, browser_cmd)
}

/// Chromium normally recovers from a stale `SingletonLock` (left behind when
/// a previous run of this profile was killed rather than closed cleanly) by
/// checking whether the PID it names is still alive on the *same hostname*.
/// This machine has no static hostname set, so its transient hostname can
/// differ between the run that wrote the lock and the run that reads it —
/// when it does, Chromium can't confirm the lock is stale and refuses to
/// open a window at all, with nothing telling the user why. Since this
/// profile is exclusively used by one isolated web app, it's safe for us to
/// clear the lock ourselves whenever the PID it names is dead or belongs to
/// an unrelated process, regardless of what hostname it was stamped with.
fn clear_stale_singleton_lock(profile: &Path) {
    let Ok(target) = std::fs::read_link(profile.join("SingletonLock")) else {
        return;
    };
    let Some(pid) = parse_lock_pid(&target.to_string_lossy()) else {
        return;
    };

    if process_owns_profile(pid, profile) {
        return;
    }

    for file_name in ["SingletonLock", "SingletonSocket", "SingletonCookie"] {
        let _ = std::fs::remove_file(profile.join(file_name));
    }
}

/// `SingletonLock` points to `<hostname>-<pid>`; the hostname itself may
/// contain hyphens, so split from the right to reliably isolate the PID.
fn parse_lock_pid(lock_target: &str) -> Option<u32> {
    let (_, pid_str) = lock_target.rsplit_once('-')?;
    pid_str.parse().ok()
}

/// Whether `pid` is alive and its command line still references `profile`
/// (as `--user-data-dir=<profile>`). A dead PID, or one reused since by an
/// unrelated process, means the lock it left behind is safe to clear.
fn process_owns_profile(pid: u32, profile: &Path) -> bool {
    let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{pid}/cmdline")) else {
        return false;
    };

    cmdline
        .split('\0')
        .filter_map(|arg| arg.strip_prefix("--user-data-dir="))
        .any(|dir| Path::new(dir) == profile)
}

/// Build the geometry flags to restore the window, or `None` when there is
/// nothing to restore or the caller already pinned the geometry (e.g. the app's
/// "start maximized" preference is on).
fn geometry_flags(profile: &Path, browser_cmd: &[String]) -> Option<Vec<String>> {
    if browser_cmd.iter().any(|arg| {
        arg == "--start-maximized"
            || arg.starts_with("--window-size")
            || arg.starts_with("--window-position")
    }) {
        return None;
    }

    let prefs = std::fs::read_to_string(profile.join("Default").join("Preferences")).ok()?;
    let json: Value = serde_json::from_str(&prefs).ok()?;
    let placement = find_placement(json.get("browser")?.get("app_window_placement")?)?;

    flags_from_placement(placement)
}

/// `app_window_placement` nests as `{ "<a>": { "<b>": { left, top, ... } } }`.
/// An isolated profile holds exactly one app, so descend to the single object
/// that actually carries the geometry keys.
fn find_placement(value: &Value) -> Option<&Map<String, Value>> {
    let object = value.as_object()?;
    if object.contains_key("left") && object.contains_key("right") {
        return Some(object);
    }
    object.values().find_map(find_placement)
}

fn flags_from_placement(placement: &Map<String, Value>) -> Option<Vec<String>> {
    if placement.get("maximized").and_then(Value::as_bool) == Some(true) {
        return Some(vec!["--start-maximized".to_string()]);
    }

    let left = placement.get("left")?.as_i64()?;
    let top = placement.get("top")?.as_i64()?;
    let right = placement.get("right")?.as_i64()?;
    let bottom = placement.get("bottom")?.as_i64()?;

    let width = right - left;
    let height = bottom - top;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(vec![
        format!("--window-size={width},{height}"),
        format!("--window-position={left},{top}"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(json: &str) -> Map<String, Value> {
        find_placement(&serde_json::from_str(json).unwrap())
            .unwrap()
            .clone()
    }

    #[test]
    fn parses_profile_and_browser_command() {
        let args = vec![
            "--profile".to_string(),
            "/tmp/p".to_string(),
            "--".to_string(),
            "brave-browser".to_string(),
            "--app=https://example.com".to_string(),
        ];
        let (profile, cmd) = parse_args(args);
        assert_eq!(profile, Some(PathBuf::from("/tmp/p")));
        assert_eq!(cmd, ["brave-browser", "--app=https://example.com"]);
    }

    #[test]
    fn parses_pid_from_lock_target_with_plain_hostname() {
        assert_eq!(parse_lock_pid("fedora-9149"), Some(9149));
    }

    #[test]
    fn parses_pid_from_lock_target_with_hyphenated_hostname() {
        assert_eq!(parse_lock_pid("DESKTOP-2F5HJ6B-7664"), Some(7664));
    }

    #[test]
    fn rejects_lock_target_without_pid() {
        assert_eq!(parse_lock_pid("not-a-pid-here-abc"), None);
        assert_eq!(parse_lock_pid("nohyphen"), None);
    }

    #[test]
    fn dead_pid_does_not_own_profile() {
        // PID 1 is always alive but never a match for this made-up profile,
        // and out-of-range PIDs have no /proc entry at all either way.
        assert!(!process_owns_profile(1, Path::new("/does/not/exist")));
        assert!(!process_owns_profile(
            u32::MAX,
            Path::new("/does/not/exist")
        ));
    }

    #[test]
    fn live_process_without_matching_user_data_dir_does_not_own_profile() {
        // This test process is alive but was never launched with
        // --user-data-dir, so it must not be mistaken for the profile owner.
        assert!(!process_owns_profile(
            std::process::id(),
            Path::new("/some/profile/path")
        ));
    }

    #[test]
    fn finds_nested_placement_leaf() {
        let map = placement(
            r#"{ "example": { "com_/": { "left": 80, "top": 80, "right": 680, "bottom": 530, "maximized": false } } }"#,
        );
        assert_eq!(
            flags_from_placement(&map),
            Some(vec![
                "--window-size=600,450".to_string(),
                "--window-position=80,80".to_string(),
            ])
        );
    }

    #[test]
    fn maximized_placement_starts_maximized() {
        let map = placement(
            r#"{ "a": { "b": { "left": 0, "top": 0, "right": 100, "bottom": 100, "maximized": true } } }"#,
        );
        assert_eq!(
            flags_from_placement(&map),
            Some(vec!["--start-maximized".to_string()])
        );
    }

    #[test]
    fn zero_area_placement_is_ignored() {
        let map = placement(
            r#"{ "a": { "b": { "left": 50, "top": 50, "right": 50, "bottom": 50, "maximized": false } } }"#,
        );
        assert_eq!(flags_from_placement(&map), None);
    }

    #[test]
    fn skips_when_geometry_already_pinned() {
        let cmd = vec!["brave-browser".to_string(), "--start-maximized".to_string()];
        assert_eq!(geometry_flags(Path::new("/nonexistent"), &cmd), None);
    }
}
