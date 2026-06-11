use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Directory this binary was built from — used to locate `.git` and
/// `update.sh` so updates operate on the same checkout that produced this
/// build. Stale if the source tree was moved or deleted after building.
const SOURCE_DIR: &str = env!("CARGO_MANIFEST_DIR");

/// Short commit hash this binary was built from, embedded by build.rs.
pub const CURRENT_COMMIT: &str = env!("VELO_GIT_HASH");

pub enum CheckResult {
    UpToDate,
    Available { local: String, remote: String },
    Unavailable(String),
}

pub enum UpdateResult {
    Success,
    Failed(String),
}

fn source_dir() -> Option<&'static Path> {
    let dir = Path::new(SOURCE_DIR);
    (dir.join(".git").exists() && dir.join("update.sh").exists()).then_some(dir)
}

/// Runs `work` on a background thread and delivers its result back on the
/// GTK main thread via `on_done`.
fn spawn_then<T, F, D>(work: F, on_done: D)
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    let slot: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    let slot_bg = Arc::clone(&slot);

    std::thread::spawn(move || {
        *slot_bg.lock().unwrap() = Some(work());
    });

    let cb = RefCell::new(Some(on_done));
    glib::timeout_add_local(Duration::from_millis(80), move || {
        if let Some(result) = slot.lock().unwrap().take() {
            if let Some(f) = cb.borrow_mut().take() {
                f(result);
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

/// Checks the git remote for new commits on the current branch.
pub fn check_for_update<F: FnOnce(CheckResult) + 'static>(on_done: F) {
    spawn_then(run_check, on_done);
}

fn run_check() -> CheckResult {
    let Some(dir) = source_dir() else {
        return CheckResult::Unavailable(
            "Velo Player's source checkout wasn't found, so it can't update itself. \
             Reinstall with install.sh to enable updates."
                .into(),
        );
    };

    let fetch = Command::new("git")
        .args(["fetch", "--quiet", "origin"])
        .current_dir(dir)
        .status();
    if !matches!(fetch, Ok(status) if status.success()) {
        return CheckResult::Unavailable(
            "Couldn't reach the git remote — check your network connection.".into(),
        );
    }

    let branch = git_output(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let local = git_output(dir, &["rev-parse", "--short", "HEAD"]);
    let remote = branch
        .as_deref()
        .and_then(|b| git_output(dir, &["rev-parse", "--short", &format!("origin/{b}")]));

    match (local, remote) {
        (Some(local), Some(remote)) if local == remote => CheckResult::UpToDate,
        (Some(local), Some(remote)) => CheckResult::Available { local, remote },
        _ => CheckResult::Unavailable("Couldn't determine the repository's update status.".into()),
    }
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Pulls the latest commit, rebuilds, and reinstalls Velo Player via `update.sh`.
pub fn run_update<F: FnOnce(UpdateResult) + 'static>(on_done: F) {
    let Some(dir) = source_dir() else {
        on_done(UpdateResult::Failed("Source checkout not found.".into()));
        return;
    };
    let dir = dir.to_path_buf();

    spawn_then(
        move || match Command::new("sh").arg("update.sh").current_dir(&dir).output() {
            Ok(out) if out.status.success() => UpdateResult::Success,
            Ok(out) => {
                let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if msg.is_empty() {
                    msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                }
                if msg.is_empty() {
                    msg = "update.sh exited with an error.".into();
                }
                UpdateResult::Failed(msg)
            }
            Err(e) => UpdateResult::Failed(e.to_string()),
        },
        on_done,
    );
}

/// Launches a fresh instance of Velo Player and exits this one.
pub fn restart() {
    let _ = Command::new("velo-player").spawn();
    std::process::exit(0);
}
