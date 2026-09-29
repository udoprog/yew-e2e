//! One browser run per build directory, including its frontend build and cleanup.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;

/// How often a waiting runner asks again for the lock. A run lasts minutes, so
/// this is far below the resolution anyone cares about and costs nothing.
const POLL: Duration = Duration::from_millis(200);

/// How often a waiting runner says it is still waiting. Silence for minutes
/// reads as a hang to whoever is watching the terminal.
const ANNOUNCE: Duration = Duration::from_secs(30);

/// **Which other runs this one is held apart from.**
///
/// Everything a run writes, it writes under [`crate::dist::target_dir`]: the
/// frontend into `e2e-dist`, the session `--last-session` reads back into
/// `e2e-sessions`, and a fetched chromedriver into `e2e`. Two runs sharing that
/// directory genuinely collide, and [`Scope::Target`] is the scope that says so
/// and no more.
///
/// Nothing else the runner touches is shared: each driver is told `--port 0`,
/// so its port is taken rather than agreed, and the browser's profile and its
/// download directory are made per session. Whatever the application under
/// test shares beyond that is for its fixture to keep apart. What is left is
/// the machine itself, and [`Scope::Project`] is how to ask for that back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub(crate) enum Scope {
    /// One run at a time per build directory, which is one per worktree unless
    /// `CARGO_TARGET_DIR` says otherwise. The default, because it is the set of
    /// files a run actually writes.
    Target,
    /// One run at a time across every worktree of this Git project. Two
    /// reasons to ask for it, neither of them correctness of the tests themselves: several
    /// cold `trunk` builds at once are heavy enough to matter on a small
    /// machine, and `~/.cache/trunk` is shared by every worktree, so a
    /// wasm-bindgen bump leaves two cold builds racing to populate it for the
    /// first time.
    Project,
    /// No lock at all. Two runs in one build directory then share `e2e-dist`
    /// while trunk is writing it and `e2e-sessions` after, so `--last-session`
    /// can pick the other run's session up as the newest.
    None,
}

/// Where `scope` keeps its lock, or `None` where it asks for no lock.
pub(crate) fn directory(scope: Scope) -> Result<Option<PathBuf>> {
    match scope {
        Scope::None => Ok(None),
        Scope::Target => Ok(Some(crate::dist::target_dir()?)),
        Scope::Project => Ok(Some(project(&crate::dist::workspace_root()?)?)),
    }
}

/// The shared Git directory, which every worktree of one project answers with
/// and which is exactly why [`Scope::Project`] reaches across them.
fn project(root: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .context("locating the shared Git directory for the e2e run lock")?;
    ensure!(
        output.status.success(),
        "cannot locate the e2e project's shared Git directory: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let directory = std::str::from_utf8(&output.stdout)
        .context("the shared Git directory is not UTF-8")?
        .trim_end_matches(['\r', '\n']);
    ensure!(
        !directory.is_empty(),
        "Git returned an empty shared directory"
    );
    Ok(PathBuf::from(directory))
}

/// Keep the file open until every browser has closed and the report is saved.
/// Never unlink it: a waiter already holds this inode, and replacing it would
/// let a later runner lock a different file. The OS releases ownership even
/// when the runner dies, so the file's continued existence is not a stale lock.
///
/// `directory` is what [`directory`] answered, `None` taking no lock, and
/// `name` is what the lock is called inside it.
///
/// `wait` caps how long a contended runner waits, `None` waiting for ever. The
/// cap is there because this wait is not the outermost deadline: runs are
/// wrapped in `timeout`, which puts the process down with SIGTERM once its
/// budget is gone. A runner that blocks silently until that lands reports the
/// peer it was waiting for as `signal 15`, so it gives up first and says so.
pub(crate) fn acquire(
    directory: Option<&Path>,
    name: &str,
    wait: Option<Duration>,
) -> Result<Option<File>> {
    let Some(directory) = directory else {
        return Ok(None);
    };

    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating {} for the e2e run lock", directory.display()))?;

    let path = directory.join(name);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("opening the e2e run lock at {}", path.display()))?;

    if held(&file, &path)? {
        return Ok(Some(file));
    }

    eprintln!("waiting for another e2e runner ({})", path.display());

    let started = Instant::now();
    let mut announced = started;

    loop {
        std::thread::sleep(POLL);

        if held(&file, &path)? {
            eprintln!(
                "got the e2e run lock after {}s",
                started.elapsed().as_secs()
            );
            return Ok(Some(file));
        }

        let waited = started.elapsed();

        if let Some(cap) = wait
            && waited >= cap
        {
            bail!(
                "gave up waiting for the e2e run lock at {} after {}s: another \
                 runner still holds it. Let it finish and start again, or ask \
                 for less -- a module or --last-session needs less budget after \
                 the wait than the whole suite does. A longer --lock-wait (0 \
                 waits for ever) only helps if the outer timeout is raised with \
                 it and there is still room to run afterwards. `--lock` says \
                 how wide this is: the default holds runs apart only when they \
                 share a build directory, so a peer in another worktree is \
                 either running `--lock project` or sharing CARGO_TARGET_DIR \
                 with you",
                path.display(),
                waited.as_secs()
            );
        }

        if announced.elapsed() >= ANNOUNCE {
            announced = Instant::now();
            eprintln!("still waiting for the e2e run lock ({}s)", waited.as_secs());
        }
    }
}

/// Whether this attempt took ownership. A lock held elsewhere is not an error.
fn held(file: &File, path: &Path) -> Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(TryLockError::WouldBlock) => Ok(false),
        Err(TryLockError::Error(error)) => {
            Err(error).with_context(|| format!("locking the e2e project at {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "yew-e2e.lock";
    use std::process::{Child, Stdio};
    use std::time::{Duration, Instant};

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    struct Runner(Child);

    impl Drop for Runner {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn child_runner() {
        let Some(directory) = std::env::var_os("YEW_E2E_LOCK_TEST_DIR") else {
            return;
        };
        let _lock = acquire(Some(Path::new(&directory)), NAME, None).unwrap();
        std::fs::write(
            std::env::var_os("YEW_E2E_LOCK_TEST_READY").unwrap(),
            "ready",
        )
        .unwrap();
        std::thread::sleep(Duration::from_secs(30));
    }

    /// Each `acquire` opens the file for itself, so two in one process contend
    /// exactly as two runners do and no child is needed to hold the lock.
    #[test]
    fn a_contended_runner_gives_up_at_its_cap_and_waits_within_it() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("target");

        let holder = acquire(Some(&directory), NAME, None).unwrap();
        let cap = Duration::from_millis(300);

        let started = Instant::now();
        let error = acquire(Some(&directory), NAME, Some(cap)).unwrap_err();
        let waited = started.elapsed();

        assert!(waited >= cap, "gave up early, after {waited:?}");
        assert!(
            waited < Duration::from_secs(10),
            "never gave up: {waited:?}"
        );
        assert!(error.to_string().contains("gave up waiting"), "{error:?}");

        // Within its cap the peer is waited out rather than reported.
        drop(holder);
        acquire(Some(&directory), NAME, Some(cap)).unwrap();
    }

    /// **The directory is made rather than required**, because the scope that
    /// names it is the build directory, and a worktree that has never been
    /// built does not have one yet.
    #[test]
    fn a_build_directory_that_is_not_there_yet_is_made() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("target/nested");

        let _lock = acquire(Some(&directory), NAME, None).unwrap();

        assert!(directory.join(NAME).is_file());
    }

    /// **Two build directories do not contend**, which is the whole of what
    /// moving the scope off the Git project bought: a worktree's runs are held
    /// apart from each other and from nobody else's.
    #[test]
    fn separate_build_directories_are_independent() {
        let temp = tempfile::tempdir().unwrap();
        let mine = temp.path().join("mine/target");
        let theirs = temp.path().join("theirs/target");

        let _held = acquire(Some(&mine), NAME, None).unwrap();
        let _also = acquire(Some(&theirs), NAME, Some(Duration::from_millis(100))).unwrap();
    }

    /// **Asking for no lock takes none**, and says so with `None` rather than
    /// handing back a file that would be unlocked when it dropped.
    #[test]
    fn no_lock_is_asked_for_and_none_is_taken() {
        assert_eq!(directory(Scope::None).unwrap(), None);
        assert!(
            acquire(None, NAME, Some(Duration::from_millis(0)))
                .unwrap()
                .is_none()
        );
    }

    /// **The default scope is not the project scope**, which is the change
    /// itself: the lock a run takes by default is in its own build directory,
    /// not in the Git directory every worktree of this project shares.
    #[test]
    fn the_default_scope_is_the_build_directory_and_not_the_project() {
        let target = directory(Scope::Target).unwrap().unwrap();
        let project = directory(Scope::Project).unwrap().unwrap();

        assert_eq!(target, crate::dist::target_dir().unwrap());
        assert_ne!(target, project);
    }

    /// **`--lock project` still reaches across worktrees**, since that is the
    /// only thing anyone would ask for it for. Git's shared directory is what
    /// makes it reach, and a different project answers with a different one.
    #[test]
    fn the_project_scope_is_shared_by_every_worktree_of_one_project() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        let linked = temp.path().join("worktree");
        let other = temp.path().join("other");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&other).unwrap();
        git(&root, &["init", "--quiet"]);
        git(
            &root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "fixture",
            ],
        );
        git(
            &root,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                linked.to_str().unwrap(),
            ],
        );
        git(&other, &["init", "--quiet"]);
        assert_eq!(project(&root).unwrap(), project(&linked).unwrap());
        assert_ne!(project(&root).unwrap(), project(&other).unwrap());
    }

    #[test]
    fn a_killed_runner_releases_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let held_by_child = temp.path().join("target");
        let elsewhere = temp.path().join("other-target");

        let ready = temp.path().join("ready");
        let mut runner = Runner(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "run_lock::tests::child_runner", "--nocapture"])
                .env("YEW_E2E_LOCK_TEST_DIR", &held_by_child)
                .env("YEW_E2E_LOCK_TEST_READY", &ready)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() {
            assert!(
                runner.0.try_wait().unwrap().is_none(),
                "child exited before locking"
            );
            assert!(Instant::now() < deadline, "child never acquired the lock");
            std::thread::sleep(Duration::from_millis(10));
        }

        let probe = OpenOptions::new()
            .read(true)
            .write(true)
            .open(held_by_child.join(NAME))
            .unwrap();
        assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
        let _independent = acquire(Some(&elsewhere), NAME, None).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiting = std::thread::spawn({
            let held_by_child = held_by_child.clone();
            move || {
                tx.send(acquire(Some(&held_by_child), NAME, None)).unwrap();
            }
        });
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
        runner.0.kill().unwrap();
        runner.0.wait().unwrap();
        let lock = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        waiting.join().unwrap();
        assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
        drop(lock);
        probe.try_lock().unwrap();
    }
}
