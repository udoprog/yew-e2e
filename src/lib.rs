//! [<img alt="github" src="https://img.shields.io/badge/github-udoprog/yew--e2e-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/yew-e2e)
//! [<img alt="crates.io" src="https://img.shields.io/crates/v/yew-e2e.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/yew-e2e)
//! [<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-yew--e2e-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/yew-e2e)
//!
//! Browser tests for web applications, and the runner that drives them.
//!
//! Written for [Yew] applications built with [Trunk], though nothing here depends
//! on either: a suite names the [`Fixture`] that starts the application under test
//! for every test, and each test is handed a [`TestDriver`] pointed at it.
//!
//! What the runner takes care of:
//!
//! * One binary and one entrypoint in place of libtest, declared with
//!   [`harness!`], running several browsers at once (`--parallel`).
//! * Firefox through `geckodriver` or Chrome through `chromedriver`, with a
//!   `chromedriver` matching the installed Chrome fetched when none is on the
//!   `PATH`.
//! * The frontend built once per run with `trunk build`, or served from `E2E_DIST`
//!   when it is already built.
//! * A record of every run, so `--last-session` runs only what did not pass.
//! * One run at a time per build directory (`--lock`).
//! * Waits that say what they were waiting for when they give up, and a report of
//!   what the page was doing when it never came up.
//!
//! <br>
//!
//! ## Example
//!
//! A test is an `async fn` taking the driver and the fixture. The fixture starts
//! the application for one test and says where the browser should go:
//!
//! ```no_run
//! use yew_e2e::prelude::*;
//! use yew_e2e::{Config, Fixture, Frontend};
//!
//! struct App {
//!     url: String,
//! }
//!
//! impl Fixture for App {
//!     type Setup = ();
//!
//!     fn config() -> Config {
//!         Config::default().frontend(Frontend::None)
//!     }
//!
//!     async fn start((): ()) -> Result<Self> {
//!         // Start the application here, on a port of its own.
//!         Ok(App { url: String::from("http://127.0.0.1:8080") })
//!     }
//!
//!     fn url(&self) -> String {
//!         self.url.clone()
//!     }
//!
//!     async fn quit(self) -> Result<()> {
//!         Ok(())
//!     }
//! }
//!
//! mod greeting {
//!     use yew_e2e::prelude::*;
//!
//!     pub async fn says_hello<F>(driver: &mut TestDriver, _: &mut F) -> Result<()> {
//!         driver.wait_texts("h1", ["Hello"]).await
//!     }
//! }
//!
//! yew_e2e::harness! {
//!     App;
//!     greeting::{says_hello},
//! }
//! ```
//!
//! The suite is registered as a test without libtest's harness:
//!
//! ```toml
//! [[test]]
//! name = "e2e"
//! path = "tests/e2e/main.rs"
//! harness = false
//! ```
//!
//! A test that needs something other than the bare fixture names it in
//! parentheses after its name, each word setting a `bool` field of the fixture's
//! `Setup`:
//!
//! ```ignore
//! yew_e2e::harness! {
//!     App;
//!     cart::{adds_an_item, removes_an_item},
//!     login::{remembers_the_user(signed_in)},
//! }
//! ```
//!
//! [`examples/static_page.rs`] is a suite that runs as it is, against a page its
//! fixture serves itself:
//!
//! ```sh
//! cargo run -p yew-e2e --example static_page
//! ```
//!
//! <br>
//!
//! ## The frontend
//!
//! By default the fixture's frontend is built once per run with `trunk build`, in
//! the workspace root, into `target/e2e-dist`. [`dist()`] says where it is, for
//! the fixture to serve. [`Frontend::Trunk(dir)`] runs it somewhere else under
//! the workspace root, and [`Frontend::None`] builds nothing, for a fixture that
//! serves its own pages.
//!
//! Everything else a run leaves behind goes under the build directory too: the
//! record of every run in `target/e2e-sessions`, the run lock, a fetched
//! `chromedriver` in `target/e2e`. Two suites sharing one build directory give
//! each other room with [`Config::sessions_dir`] and [`Config::lock_name`].
//!
//! <br>
//!
//! ## Running
//!
//! ```sh
//! cargo test -p app-e2e                      # everything
//! cargo test -p app-e2e -- cart::            # everything whose name contains cart::
//! cargo test -p app-e2e -- --exact cart::adds_an_item
//! cargo test -p app-e2e -- --list            # the names, one per line, on stdout
//! cargo test -p app-e2e -- --last-session    # what did not pass last time
//! ```
//!
//! | Flag | |
//! |---|---|
//! | `-j`, `--parallel N` | How many tests run at once, each in its own browser. Defaults to 2. |
//! | `--headed` | Show the browsers. |
//! | `--browser firefox\|chrome` | Which browser to drive. Detected where not given. |
//! | `--exact` | Filters match whole names rather than parts of them. |
//! | `--list` | List the selected tests and run nothing. |
//! | `--allow-empty` | A run that selects nothing passes rather than fails. |
//! | `--last-session`, `--session NAME` | Run only what did not pass in that session. |
//! | `--timeout SECONDS` | How long one test may run. Defaults to 30; 0 waits for ever. |
//! | `--grace SECONDS` | How long a running test is given to finish after Ctrl-C or SIGTERM. |
//! | `--lock target\|project\|none` | Which other runs this one waits for: those in the same build directory (the default), every worktree of the Git project, or none. |
//! | `--lock-wait SECONDS` | How long to wait for that lock. |
//!
//! | Variable | |
//! |---|---|
//! | `E2E_HEADED` | As `--headed`. |
//! | `E2E_BROWSER` | As `--browser`. |
//! | `E2E_DIST` | A frontend that is already built, served in place of building one. |
//! | `E2E_THEME` | `dark` or `light`, for the colour scheme the browser reports (Firefox). |
//! | `E2E_MOTION` | `reduce` or `normal`, for the motion preference the browser reports (Firefox). |
//! | `E2E_SCALE` | The device pixel ratio the browser reports. |
//! | `E2E_SHOTS` | Where [`TestDriver::snapshot`] writes; relative to `target/e2e-shots`. |
//!
//! [`Config::lock_name`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.lock_name
//! [`Config::sessions_dir`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.sessions_dir
//! [`dist()`]: https://docs.rs/yew-e2e/latest/yew_e2e/fn.dist.html
//! [`examples/static_page.rs`]: https://github.com/udoprog/yew-e2e/blob/main/examples/static_page.rs
//! [`Fixture`]: https://docs.rs/yew-e2e/latest/yew_e2e/trait.Fixture.html
//! [`Frontend::None`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.None
//! [`Frontend::Trunk(dir)`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.Trunk
//! [`harness!`]: https://docs.rs/yew-e2e/latest/yew_e2e/macro.harness.html
//! [`TestDriver::snapshot`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.TestDriver.html#method.snapshot
//! [`TestDriver`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.TestDriver.html
//! [Trunk]: https://trunkrs.dev
//! [Yew]: https://yew.rs

#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::fmt::Write;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail, ensure};
use futures_util::FutureExt;
use thirtyfour::common::types::ElementRect;
use thirtyfour::prelude::*;
use tokio::time::{self, Duration};

mod dist;
mod driver;
mod fetch;
pub mod harness;
#[cfg(test)]
mod lib_tests;
mod run_lock;
mod sandbox;
mod sessions;

pub use self::dist::{dist, target_dir, workspace_root};
pub use self::driver::Engine;

use self::driver::{Driver, QUIT_TIMEOUT, STARTUP_TIMEOUT, with_cleanup};

/// What a test file needs in scope, in one `use yew_e2e::prelude::*;`.
///
/// That is [`anyhow`](mod@anyhow)'s [`Result`] and error macros, the whole of
/// `thirtyfour`'s prelude (among it [`By`], [`Key`] and [`WebDriver`]), and
/// the [`TestDriver`] and [`TestElement`] a test is handed.
///
/// [`Result`]: anyhow::Result
/// [`By`]: thirtyfour::By
/// [`Key`]: thirtyfour::Key
/// [`WebDriver`]: thirtyfour::WebDriver
pub mod prelude {
    pub use anyhow::{Context, Result, anyhow, bail, ensure};
    pub use thirtyfour::prelude::*;

    /// What every test is handed, so its signature can be written without a
    /// `use` per file saying so.
    pub use crate::{TestDriver, TestElement};
}

/// **The application under test**, started afresh for every test and taken
/// down after it.
///
/// A suite names one fixture in [`harness!`], and every test is handed the one
/// started for it beside the browser, so whatever a test needs to reach behind
/// the page (a database, a port, a channel into the server) is its own.
pub trait Fixture: Sized + 'static {
    /// What one test asks of the fixture started for it. [`harness!`] sets a
    /// `bool` field of this for every word in the parentheses after a test's
    /// name; `()` where no test asks for anything.
    type Setup: Default;

    /// How this suite is run: its name, its frontend and its lock.
    fn config() -> Config {
        Config::default()
    }

    /// Install the run's tracing subscriber. The default prints `INFO` and
    /// above.
    fn init_tracing() {
        _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();
    }

    /// Confine the application to `root`, a directory made fresh for this run
    /// under the build directory, before any fixture starts. The run fails if
    /// anything new is under `root` when it finishes; see [`Fixture::check`]
    /// for failing the one test that reached outside it.
    fn enter_sandbox(root: &Path) -> Result<()> {
        _ = root;
        Ok(())
    }

    /// Asked after every test: whatever the application did during it that
    /// fails the test even though the test itself passed.
    fn check() -> Result<()> {
        Ok(())
    }

    /// Start the application for one test.
    fn start(setup: Self::Setup) -> impl Future<Output = Result<Self>>;

    /// Where the browser is pointed for this test.
    fn url(&self) -> String;

    /// Take the application down once its test has finished.
    fn quit(self) -> impl Future<Output = Result<()>>;
}

/// How a suite is run; see [`Fixture::config`].
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) about: String,
    pub(crate) frontend: Frontend,
    pub(crate) lock: String,
    pub(crate) sessions: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            about: String::from("A browser suite."),
            frontend: Frontend::Trunk(PathBuf::new()),
            lock: String::from("yew-e2e.lock"),
            sessions: String::from("e2e-sessions"),
        }
    }
}

impl Config {
    /// The one line `--help` opens with.
    pub fn about(mut self, about: impl Into<String>) -> Self {
        self.about = about.into();
        self
    }

    /// What the suite serves, and how it is built.
    pub fn frontend(mut self, frontend: Frontend) -> Self {
        self.frontend = frontend;
        self
    }

    /// What the run lock is called inside whichever directory `--lock` names.
    pub fn lock_name(mut self, name: impl Into<String>) -> Self {
        self.lock = name.into();
        self
    }

    /// Where under the build directory the record of every run is kept, which
    /// is what `--last-session` reads the newest of. Two suites sharing a
    /// build directory name one each, or each picks up the other's runs.
    pub fn sessions_dir(mut self, name: impl Into<String>) -> Self {
        self.sessions = name.into();
        self
    }
}

/// The frontend a suite serves, built once for the whole run; [`dist`] is
/// where it ends up.
#[derive(Debug, Clone)]
pub enum Frontend {
    /// Nothing to build: the fixture serves its own pages.
    None,
    /// `trunk build` run in this directory, relative to the workspace root,
    /// into `e2e-dist` under the build directory. `E2E_DIST` names a frontend
    /// already built in its place.
    Trunk(PathBuf),
}

/// **What a test last asked to wait for**, in the words the test used.
///
/// A test that is given up on has to say what it was doing, and how long it
/// took says nothing: the run is left reading a name and a number of seconds
/// against a suite of tests that all wait on something. One browser carries one
/// test at a time, so this hangs off the browser's driver and is that test's
/// own; it is cleared as each test begins so that a test which never waited on
/// anything does not inherit the last one's words.
#[derive(Clone, Default)]
pub(crate) struct Waiting(Arc<Mutex<Option<String>>>);

impl Waiting {
    /// Note what is being waited for now.
    fn on(&self, what: impl std::fmt::Display) {
        *self.held() = Some(what.to_string());
    }

    /// Forget it, which is what the start of a test does.
    fn clear(&self) {
        *self.held() = None;
    }

    /// **The clause a report hangs off a timeout**, and nothing at all where
    /// the test never named a wait.
    pub(crate) fn clause(&self) -> String {
        match &*self.held() {
            Some(what) => format!(", last waiting for {what}"),
            None => String::new(),
        }
    }

    /// A panic while a test held this says nothing about the words in it, which
    /// are only ever replaced whole.
    fn held(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.0.lock().unwrap_or_else(|held| held.into_inner())
    }
}

/// One browser, kept for as long as there are tests to run in it.
pub(crate) struct Browser {
    webdriver: Driver,
    driver: TestDriver,
}

impl Browser {
    /// Start a WebDriver server and the one browser it drives.
    pub(crate) async fn open() -> Result<Self> {
        let engine = crate::harness::engine();
        let deadline = time::Instant::now() + STARTUP_TIMEOUT;
        let caps = engine.capabilities(crate::harness::headed())?;
        let webdriver = Driver::new(engine, deadline).await?;
        Self::connect(webdriver, caps, deadline).await
    }

    async fn connect(
        mut webdriver: Driver,
        mut caps: Capabilities,
        deadline: time::Instant,
    ) -> Result<Self> {
        let downloads = tempfile::tempdir().context("creating browser download directory")?;
        let directory = downloads.path().to_string_lossy().into_owned();
        let key = match webdriver.engine() {
            Engine::Firefox => "moz:firefoxOptions",
            Engine::Chrome => "goog:chromeOptions",
        };
        if !caps.contains_key(key) {
            caps.set(key, serde_json::json!({}))?;
        }
        match webdriver.engine() {
            Engine::Firefox => {
                let options = caps
                    .get_mut("moz:firefoxOptions")
                    .context("Firefox capabilities have no options")?;
                let prefs = options
                    .as_object_mut()
                    .context("Firefox options")?
                    .entry("prefs")
                    .or_insert_with(|| serde_json::json!({}));
                let prefs = prefs.as_object_mut().context("Firefox preferences")?;
                prefs.insert("browser.download.folderList".into(), 2.into());
                prefs.insert("browser.download.dir".into(), directory.into());
                prefs.insert("browser.download.useDownloadDir".into(), true.into());
                prefs.insert(
                    "browser.helperApps.neverAsk.saveToDisk".into(),
                    "audio/wav,application/zip,application/json".into(),
                );
            }
            Engine::Chrome => {
                let options = caps
                    .get_mut("goog:chromeOptions")
                    .context("Chrome capabilities have no options")?;
                options["prefs"] = serde_json::json!({
                    "download.default_directory": directory,
                    "download.prompt_for_download": false,
                });
            }
        }
        tracing::info!(
            "{} address: {:?}",
            webdriver.engine().driver(),
            webdriver.address
        );

        let url = format!("http://{}", webdriver.address);
        let connected = webdriver
            .during(deadline, "startup", async {
                Ok(WebDriver::new(&url, caps).await?)
            })
            .await;
        let driver = match connected {
            Ok(driver) => driver,
            Err(error) => return Err(with_cleanup(error, webdriver.terminate().await)),
        };

        Ok(Self {
            webdriver,
            driver: TestDriver {
                inner: driver,
                wait_timeout: WAIT_TIMEOUT,
                load_timeout: LOAD_TIMEOUT,
                waiting: Waiting::default(),
                downloads,
            },
        })
    }

    /// Put the browser on a fresh tab, then go to `url`.
    ///
    /// The preceding document is closed rather than navigated away from. A
    /// WebDriver navigation leaves session history behind; long runs showed
    /// memory growth that closing the previous tab removed.
    async fn reset(&mut self, url: &str) -> Result<()> {
        self.driver.waiting.clear();
        for file in std::fs::read_dir(self.driver.downloads.path())? {
            std::fs::remove_file(file?.path())?;
        }
        let old = self.driver.inner.window().await?;
        let fresh = self.driver.inner.new_tab().await?;
        self.driver.inner.switch_to_window(fresh.clone()).await?;
        self.driver.inner.switch_to_window(old).await?;
        self.driver.inner.close_window().await?;
        self.driver.inner.switch_to_window(fresh).await?;
        self.driver.set_window_size(WINDOW.0, WINDOW.1).await?;
        self.driver.inner.goto(url).await?;
        Ok(())
    }

    /// Take the browser and its driver down.
    ///
    /// A quit that only timed out comes back as `Ok(Some(_))` once the
    /// driver's process group is reaped: nothing is left running, so it is a
    /// warning for the caller to say rather than a failure.
    pub(crate) async fn close(mut self) -> Result<Option<anyhow::Error>> {
        // Keep the original handle alive until Drop marks it leaked. Otherwise
        // cancelling quit would invoke thirtyfour's synchronous fallback before
        // the timeout future could return and before we could kill the driver.
        let driver = self.driver.inner.clone();
        let quit = self
            .webdriver
            .during(time::Instant::now() + QUIT_TIMEOUT, "quit", async {
                Ok(driver.quit().await?)
            })
            .await;
        let reaped = self.webdriver.terminate().await;
        match (quit, reaped) {
            (Ok(()), reaped) => reaped.map(|()| None),
            (Err(error), Ok(())) if error.downcast_ref::<driver::TimedOut>().is_some() => {
                Ok(Some(error))
            }
            (Err(error), reaped) => Err(with_cleanup(error, reaped)),
        }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        // `leak` marks the shared session as already quit; it leaks no memory.
        // Our Driver owns process teardown, including cancellation and crashes.
        let _ = self.driver.inner.clone().leak();
    }
}

#[cfg(test)]
mod browser_tests {
    use anyhow::{Context, Result, ensure};

    use super::Browser;

    /// Each test gets a document with no predecessor retained in its browser.
    ///
    /// This uses the real WebDriver because only a browser can say which tabs
    /// and history entries survived. Replacing `Browser::reset` with a plain
    /// `goto` makes the second URL retain the first tab and add a history entry.
    #[tokio::test]
    #[ignore = "requires a real WebDriver"]
    async fn a_reset_discards_the_preceding_document() -> Result<()> {
        let mut browser = Browser::open().await?;

        let result = async {
            browser.reset("data:text/html,first").await?;
            let first = browser.driver.inner.window().await?;

            browser.reset("data:text/html,second").await?;
            let second = browser.driver.inner.window().await?;
            let windows = browser.driver.inner.windows().await?;
            let history = browser
                .driver
                .inner
                .execute("return history.length", Vec::new())
                .await?
                .json()
                .as_u64()
                .context("browser history length")?;

            ensure!(first != second, "the second reset kept the first tab");
            ensure!(windows == [second], "reset left tabs behind: {windows:?}");
            ensure!(history <= 2, "the fresh tab has {history} history entries");
            Ok(())
        }
        .await;

        match (result, browser.close().await) {
            (Ok(()), Ok(_)) => Ok(()),
            (Err(error), Ok(_)) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => Err(super::driver::with_cleanup(error, Err(cleanup))),
        }
    }
}

/// The window every test starts with; see [`Browser::reset`].
const WINDOW: (u32, u32) = (1600, 1000);

/// The words a session or a connection that is no longer there says, matched
/// against every link of an error's chain. `invalid session id` is the
/// WebDriver protocol's own answer to a command for a session that is gone,
/// and `session id is invalid` is thirtyfour's wrapper over it; the page
/// crash and the terminated session are the two ways the session dies without
/// a command naming it; the last two are this side of the connection - the
/// runner's own answer when the driver process has ended, and what the HTTP
/// client answers once that driver cannot be reached. Anything else an error
/// can say stays an ordinary failure, because a session death is the one
/// failure here that is not the test's.
const SESSION_DEATH: &[&str] = &[
    "invalid session id",
    "session id is invalid",
    "session deleted because of page crash",
    "session terminated",
    "the webdriver server's output ended",
    "error sending request",
];

/// Whether an error is the browser or its driver being gone, and the words
/// that said so.
pub(crate) fn browser_death(error: &anyhow::Error) -> Option<&'static str> {
    let mut source: Option<&dyn std::error::Error> = Some(error.as_ref());

    while let Some(error) = source {
        let said = error.to_string().to_lowercase();

        if let Some(mark) = SESSION_DEATH.iter().find(|mark| said.contains(**mark)) {
            return Some(mark);
        }

        source = error.source();
    }

    None
}

/// The one error a dead browser reports: what said the session was gone, and
/// what the driver had to say for itself, which the runner was reading all
/// along. The ordinary report is what a session death otherwise reads as -
/// whatever the test was last waiting for - which is how one dead browser
/// becomes two wrong tests.
fn went_away(browser: &Browser, mark: &str) -> anyhow::Error {
    let name = browser.webdriver.engine().driver();

    anyhow!(
        "the browser went away ({mark}); {name} last said: {}",
        driver_said(browser)
    )
}

/// The tail of what the driver said, stderr first, for the one line the report
/// has room for.
fn driver_said(browser: &Browser) -> String {
    let said = browser.webdriver.diagnostics();

    said.lines()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Start the fixture for one test, run it, and take the fixture down again.
pub(crate) async fn run_one<F: Fixture>(
    browser: &mut Browser,
    start: crate::harness::Start<F>,
    setup: F::Setup,
    limit: Option<Duration>,
) -> Result<()> {
    let mut fixture = F::start(setup).await?;

    let mut errors = String::new();

    let result = match browser.reset(&fixture.url()).await {
        Ok(()) => drive_one(browser, &mut fixture, start, limit).await,
        Err(error) => Err(error),
    };

    let fixture_result = fixture.quit().await;

    if let Err(error) = result.as_ref() {
        if let Some(mark) = browser_death(error) {
            let mut error = went_away(browser, mark);

            if let Err(quit) = fixture_result {
                error = error.context(format!("the fixture also failed to quit: {quit}"));
            }

            return Err(error);
        }

        writeln!(errors, "Test error: {error}")?;

        let mut source = error.source();

        while let Some(e) = source.take() {
            writeln!(errors, "  Caused by: {e}")?;
            source = e.source();
        }
    }

    if let Err(error) = fixture_result {
        writeln!(errors, "Fixture error: {error}")?;

        let mut source = error.source();

        while let Some(e) = source.take() {
            writeln!(errors, "  Caused by: {e}")?;
            source = e.source();
        }
    }

    if let Err(error) = F::check() {
        writeln!(errors, "Check error: {error}")?;
    }

    if !errors.is_empty() {
        return Err(anyhow!("{errors}"));
    }

    Ok(())
}

/// **The test itself, against `--timeout` and with the driver's output being
/// read the whole time.**
///
/// The cap is here rather than around the whole run so that a test which
/// overruns is still followed by the fixture being taken down: a run that
/// dropped the fixture where it stood would leave its application serving for
/// the rest of the suite. What the report says is the same either way, and names
/// what the test was last waiting for, which is the only thing on hand to say
/// where it stopped.
async fn drive_one<F: Fixture>(
    browser: &mut Browser,
    fixture: &mut F,
    start: crate::harness::Start<F>,
    limit: Option<Duration>,
) -> Result<()> {
    let mut timeout = pin!(async move {
        match limit {
            Some(limit) => time::sleep(limit).await,
            None => std::future::pending::<()>().await,
        }
    });

    let Browser { webdriver, driver } = browser;

    let waiting = driver.waiting.clone();

    let outcome = {
        let task = AssertUnwindSafe(start(driver, fixture)).catch_unwind();
        let mut task = pin!(task);

        loop {
            tokio::select! {
                _ = timeout.as_mut() => {
                    break Err(overran(limit, &waiting));
                }
                result = task.as_mut() => {
                    break Ok(result);
                }
                result = webdriver.io() => {
                    if !result? {
                        break Err("the WebDriver server's output ended".to_string());
                    }
                }
            }
        }
    };

    match outcome {
        Ok(Ok(Ok(()))) => match driver.error_seen().await? {
            Some(threw) => bail!("the page threw: {threw}"),
            None => Ok(()),
        },
        Ok(Ok(result)) => result,
        Ok(Err(panicked)) => bail!("the test panicked: {}", panic_message(&panicked)),
        Err(error) => bail!(error),
    }
}

/// **What a test that ran out of time is reported as.** The runner and the run
/// itself both have a way of giving up on a test, and both say it this way.
pub(crate) fn overran(limit: Option<Duration>, waiting: &Waiting) -> String {
    let seconds = limit.map_or(0, |limit| limit.as_secs());

    format!(
        "the test was still running after {seconds}s, which is --timeout{}",
        waiting.clause()
    )
}

/// What a caught panic said, for the one line the report has room for.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> &str {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return message;
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return message;
    }

    "a payload that is neither a string nor a &str"
}

/// How often a pending observation is repeated.
pub const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// How long any one observation of a page that is already up may take to agree.
const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the first look at a page may take: the browser starting, the bundle
/// being fetched, the wasm compiled and the application connected, none of
/// which has happened when a test first asks for the page; see
/// [`TestDriver::load_timeout`].
const LOAD_TIMEOUT: Duration = Duration::from_secs(15);

macro_rules! common {
    () => {
        /// Repeat `condition` until it holds, or the wait timeout passes.
        pub async fn wait_until(
            &self,
            what: impl std::fmt::Display,
            condition: impl AsyncFnMut() -> Result<bool>,
        ) -> Result<()> {
            self.wait_for(self.wait_timeout, what, condition).await
        }

        /// [`Self::wait_until`] where the default deadline is the wrong one,
        /// because what is being waited for has a length of its own that is
        /// known and is close to it. A wait that habitually lands just inside
        /// its cap is not passing, it is winning a race.
        pub async fn wait_until_within(
            &self,
            timeout: Duration,
            what: impl std::fmt::Display,
            condition: impl AsyncFnMut() -> Result<bool>,
        ) -> Result<()> {
            self.wait_for(timeout, what, condition).await
        }

        /// [`Self::wait_until`] against a deadline of the caller's choosing.
        async fn wait_for(
            &self,
            timeout: Duration,
            what: impl std::fmt::Display,
            mut condition: impl AsyncFnMut() -> Result<bool>,
        ) -> Result<()> {
            let deadline = time::Instant::now() + timeout;

            self.waiting.on(&what);

            loop {
                if condition().await? {
                    return Ok(());
                }

                if time::Instant::now() >= deadline {
                    bail!("Timed out after {timeout:?} waiting for {what}");
                }

                time::sleep(POLL_INTERVAL).await;
            }
        }

        /// Wait until the text of every node matching `by` reads `expected`.
        pub async fn wait_texts<const N: usize>(
            &self,
            by: impl IntoBy,
            expected: [&str; N],
        ) -> Result<()> {
            let by = by.into_by();

            tracing::trace!(?by, ?expected, "Waiting for texts");

            let seen = std::cell::RefCell::new(Vec::new());

            let result = self
                .wait_until(format_args!("{by} to read {expected:?}"), async || {
                    let texts = self.find_all_texts(by.clone()).await?;
                    let matched = texts == expected;
                    *seen.borrow_mut() = texts;
                    Ok(matched)
                })
                .await;

            match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    let seen = seen.into_inner();
                    Err(error.context(format!("last read {seen:?}")))
                }
            }
        }

        /// Wait until exactly `expected` nodes match `by`.
        ///
        /// **Exact equality, polled, starting immediately -- which is a trap
        /// directly after an action.** A call whose `expected` is the count that
        /// was already on screen is answered by the very first poll, before the
        /// action it is meant to be checking can have landed, so it asserts only
        /// that the page has not caught up yet. It stays green for months and
        /// then turns into a hard timeout the first time anything, however
        /// unrelated, makes the page a few milliseconds slower to lay out --
        /// and blames that change. Where the count is supposed to have *changed*,
        /// read it before the action and say so with [`Self::wait_count_from`],
        /// which refuses the degenerate case instead of passing it. Plain
        /// `wait_count` belongs where a count is being established rather than
        /// changed: a page just loaded, a selector first asserted.
        pub async fn wait_count(&self, by: impl IntoBy, expected: usize) -> Result<()> {
            let by = by.into_by();

            tracing::trace!(?by, expected, "Waiting for count");

            self.wait_until(format_args!("{by} to match {expected} nodes"), async || {
                Ok(self.find_all(by.clone()).await?.len() == expected)
            })
            .await
        }

        /// How many nodes match `by` right now, without waiting for any
        /// particular number of them. The before-read [`Self::wait_count_from`]
        /// asks for.
        #[inline]
        pub async fn count(&self, by: impl IntoBy) -> Result<usize> {
            Ok(self.find_all(by.into_by()).await?.len())
        }

        /// Wait until exactly `expected` nodes match `by`, having been told the
        /// `before` count that the action in between was supposed to change.
        ///
        /// **The before-read is the whole point.** [`Self::wait_count`] cannot
        /// tell "the action landed" apart from "the action has not started",
        /// because both of those look exactly like the number it was given; this
        /// can, because it knows which number meant "not yet". When `before`
        /// already equals `expected` it refuses outright rather than passing on
        /// the first poll, so the assertion that verifies nothing fails loudly
        /// the day it is written instead of years later on somebody else's
        /// change. Take `before` with [`Self::count`] immediately before the
        /// action -- at the top of the test it is a different question, and
        /// anything landing in between makes it a lie.
        pub async fn wait_count_from(
            &self,
            by: impl IntoBy,
            before: usize,
            expected: usize,
        ) -> Result<()> {
            let by = by.into_by();

            ensure!(
                before != expected,
                "Waiting for {by} to match {expected} nodes verifies nothing: \
                 {expected} is also the count read before the action, so the \
                 first poll agrees whether or not the action has landed. Assert \
                 the count this action is supposed to produce, or if it really \
                 is meant to leave the count alone, assert what it did change.",
            );

            tracing::trace!(?by, before, expected, "Waiting for count to change");

            self.wait_until(
                format_args!("{by} to go from {before} to {expected} nodes"),
                async || Ok(self.find_all(by.clone()).await?.len() == expected),
            )
            .await
        }

        /// Find the text contents of all nodes match the given [`By`].
        #[inline]
        pub async fn find_all_texts(&self, by: impl IntoBy) -> Result<Vec<String>> {
            let mut texts = Vec::new();

            for item in self.find_all(by.into_by()).await? {
                texts.push(item.text().await?);
            }

            Ok(texts)
        }

        /// Find an element by xpath.
        #[inline]
        pub async fn find_one_by_xpath(&self, xpath: &str) -> Result<TestElement> {
            self.find_one_by(By::XPath(xpath)).await
        }

        /// Find the one element matching `by`, waiting for it to appear.
        ///
        /// **Refusing a second match is the point.** A selector that was meant
        /// to name one thing and now names three is the way a guard goes quiet,
        /// so this says so instead of picking. Where several *is* what the page
        /// is, the question was never "the one": see [`Self::find_nth`].
        pub async fn find_one_by(&self, by: impl IntoBy) -> Result<TestElement> {
            let by = by.into_by();

            tracing::trace!(?by, "Finding one element");

            let deadline = time::Instant::now() + self.wait_timeout;

            loop {
                let items = self.find_all(by.clone()).await?;

                ensure!(
                    items.len() <= 1,
                    "Expected exactly one by {by}, but got {}: where several is \
                     what the page is, ask for one of them by index with \
                     find_nth, or for the first of them with find_first",
                    items.len()
                );

                if let Some(item) = items.into_iter().next() {
                    return Ok(item);
                }

                if time::Instant::now() >= deadline {
                    bail!(
                        "Timed out after {:?} waiting for {by}",
                        self.wait_timeout
                    );
                }

                time::sleep(POLL_INTERVAL).await;
            }
        }

        /// Find the `index`th element matching `by`, waiting for that many to
        /// appear.
        ///
        /// **What a test asks when several is the answer**, and the reading twin
        /// of [`TestDriver::press_nth`]: the row a test appended is the one at
        /// the end, three related buttons are three whatever is measured, and
        /// neither is a broken selector for [`Self::find_one_by`] to refuse.
        /// Asked here rather than by reaching for [`Self::find_all`] and
        /// indexing the answer, which drops the wait along with the sentence
        /// saying which one was wanted.
        ///
        /// A wait polling for an element to *arrive* still wants
        /// [`Self::find_all`]: nothing there is an answer to that question, and
        /// this has a deadline of its own to spend before it says so.
        pub async fn find_nth(&self, by: impl IntoBy, index: usize) -> Result<TestElement> {
            let by = by.into_by();

            tracing::trace!(?by, index, "Finding one of several elements");

            let deadline = time::Instant::now() + self.wait_timeout;

            loop {
                if let Some(item) = self.find_all(by.clone()).await?.into_iter().nth(index) {
                    return Ok(item);
                }

                if time::Instant::now() >= deadline {
                    bail!(
                        "Timed out after {:?} waiting for {by} at {index}",
                        self.wait_timeout
                    );
                }

                time::sleep(POLL_INTERVAL).await;
            }
        }

        /// The first element matching `by`, however many there turn out to be.
        /// [`Self::find_nth`] says when that is the question.
        #[inline]
        pub async fn find_first(&self, by: impl IntoBy) -> Result<TestElement> {
            self.find_nth(by, 0).await
        }

        /// Find all elements matching `by`.
        pub async fn find_all(&self, by: impl Into<By>) -> Result<Vec<TestElement>> {
            self.find_all_within(self.wait_timeout, by).await
        }

        /// [`Self::find_all`] with a bound of the caller's choosing on the call
        /// itself, for putting the question to a browser which is still
        /// starting. See [`TestDriver::load_timeout`].
        pub async fn find_all_within(
            &self,
            timeout: Duration,
            by: impl Into<By>,
        ) -> Result<Vec<TestElement>> {
            let by = by.into();
            tracing::trace!(?by, "Finding all elements");

            let task = self.inner.find_all(by.clone());
            let sleep = time::sleep(timeout);

            tokio::select! {
                _ = sleep => {
                    Err(anyhow!("find_elements({by:?}) timed out after {timeout:?}"))
                }
                result = task => {
                    Ok(result?.into_iter().map(|inner| TestElement { inner, wait_timeout: self.wait_timeout, waiting: self.waiting.clone() }).collect::<Vec<_>>())
                }
            }
        }
    }
}

/// A canvas's own reported size -- see [`TestDriver::canvas_size`].
#[derive(Debug, Clone, Copy)]
pub struct CanvasSize {
    /// The backing buffer's width, in device pixels.
    pub device_width: f64,
    /// The backing buffer's height, in device pixels.
    pub device_height: f64,
    /// The element's own layout box width, in CSS pixels.
    pub css_width: f64,
    /// The element's own layout box height, in CSS pixels.
    pub css_height: f64,
    /// What the browser itself reports for `window.devicePixelRatio`.
    pub device_pixel_ratio: f64,
}

/// A wrapped WebElement.
pub struct TestElement {
    inner: WebElement,
    wait_timeout: Duration,
    waiting: Waiting,
}

impl TestElement {
    /// The WebDriver element underneath, for what this wrapper has no method
    /// for.
    pub fn element(&self) -> &WebElement {
        &self.inner
    }

    /// Click the element.
    pub async fn click(&self) -> Result<()> {
        tracing::trace!("Clicking element");
        self.inner.click().await?;
        Ok(())
    }

    /// **Press the element with control held**, which is how a selection is
    /// commonly extended by one item.
    pub async fn extend_click(&self) -> Result<()> {
        tracing::trace!("Extend-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .key_down(thirtyfour::Key::Control)
            .click_element(&self.inner)
            .key_up(thirtyfour::Key::Control)
            .perform()
            .await?;

        Ok(())
    }

    /// **Press the element with shift held**, which picks the run from the
    /// last item picked to this one in a list that has an order.
    pub async fn shift_click(&self) -> Result<()> {
        tracing::trace!("Shift-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .key_down(thirtyfour::Key::Shift)
            .click_element(&self.inner)
            .key_up(thirtyfour::Key::Shift)
            .perform()
            .await?;

        Ok(())
    }

    /// Rest the pointer on the element without pressing anything.
    pub async fn hover(&self) -> Result<()> {
        tracing::trace!("Hovering element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .perform()
            .await?;

        Ok(())
    }

    /// Press the element twice, as one gesture.
    pub async fn double_click(&self) -> Result<()> {
        tracing::trace!("Double-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .double_click_element(&self.inner)
            .perform()
            .await?;

        Ok(())
    }

    /// **The same, a given offset from the element's centre**, for one element
    /// covering a whole surface, where on it a gesture lands being what the
    /// gesture is about.
    pub async fn double_click_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Double-clicking element off centre");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .move_by_offset(dx, dy)
            .double_click()
            .perform()
            .await?;

        Ok(())
    }

    /// **Press the element once, a given offset from its centre.** A slider is
    /// one element covering its whole travel, so where on it a press lands is
    /// the whole of what the press says.
    pub async fn click_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Clicking element off centre");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .move_by_offset(dx, dy)
            .click()
            .perform()
            .await?;

        Ok(())
    }

    /// Right-click the element.
    pub async fn context_click(&self) -> Result<()> {
        tracing::trace!("Context-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .context_click_element(&self.inner)
            .perform()
            .await?;

        Ok(())
    }

    /// Press on this element and drag `dx`, `dy` from where it is, **leaving
    /// the button down**.
    pub async fn drag_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Dragging element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .click_and_hold()
            .move_by_offset(dx.signum() * 8, dy.signum() * 8)
            .move_by_offset(dx, dy)
            .perform()
            .await?;

        Ok(())
    }

    /// The same begun at a point of the element's own choosing rather than at
    /// its centre, `at_x` and `at_y` being pixels from that centre.
    ///
    /// A sweep that has to begin in one row of a grid cannot begin wherever the
    /// element happens to be halved.
    pub async fn drag_from_by(&self, at_x: i64, at_y: i64, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(at_x, at_y, dx, dy, "Dragging from a point in an element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .move_by_offset(at_x, at_y)
            .click_and_hold()
            .move_by_offset(dx.signum() * 8, dy.signum() * 8)
            .move_by_offset(dx, dy)
            .perform()
            .await?;

        Ok(())
    }

    /// The same with `alt` held.
    pub async fn alt_drag_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Dragging element with alt held");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .key_down(thirtyfour::Key::Alt)
            .move_to_element_center(&self.inner)
            .click_and_hold()
            .move_by_offset(dx.signum() * 8, dy.signum() * 8)
            .move_by_offset(dx, dy)
            .perform()
            .await?;

        Ok(())
    }

    /// Get the text contents for this element.
    pub async fn text(&self) -> Result<String> {
        tracing::trace!("Getting element text");
        Ok(self.inner.text().await?)
    }

    /// One of the element's DOM properties.
    pub async fn prop(&self, name: &str) -> Result<String> {
        tracing::trace!(name, "Getting element property");
        Ok(self.inner.prop(name).await?.unwrap_or_default())
    }

    /// One of the element's DOM *attributes*, or the empty string when it has
    /// none.
    pub async fn attr(&self, name: &str) -> Result<String> {
        tracing::trace!(name, "Getting element attribute");
        Ok(self.inner.attr(name).await?.unwrap_or_default())
    }

    /// Bring the element into view.
    pub async fn scroll_into_view(&self) -> Result<()> {
        tracing::trace!("Scrolling element into view");
        self.inner.scroll_into_view().await?;
        Ok(())
    }

    /// Put keyboard focus here without pressing anything.
    pub async fn focus(&self) -> Result<()> {
        tracing::trace!("Focusing element");
        self.inner.focus().await?;
        Ok(())
    }

    /// Whether the element is displayed, as WebDriver judges it: present in
    /// the page is not enough, it must also not be hidden by CSS
    /// (`display: none`, `visibility: hidden`) or have no size.
    ///
    /// Answers once, at the time of asking; it does not wait.
    pub async fn visible(&self) -> Result<bool> {
        Ok(self.inner.is_displayed().await?)
    }

    /// Where this element is on screen, and how big it is.
    pub async fn rect(&self) -> Result<ElementRect> {
        tracing::trace!("Measuring element");
        Ok(self.inner.rect().await?)
    }

    /// One of the element's *computed* CSS properties.
    pub async fn css(&self, name: &str) -> Result<String> {
        tracing::trace!(name, "Getting element style");
        Ok(self.inner.css_value(name).await?)
    }

    /// The element's `value` property.
    pub async fn value(&self) -> Result<String> {
        tracing::trace!("Getting element value");
        Ok(self.inner.prop("value").await?.unwrap_or_default())
    }

    /// Clear the element contents.
    pub async fn clear(&self) -> Result<()> {
        tracing::trace!("Clearing element text");
        self.inner.clear().await?;
        Ok(())
    }

    /// Send the specified input targeting the element.
    pub async fn send_keys(&self, keys: &str) -> Result<()> {
        tracing::trace!(?keys, "Sending keys to element");
        self.inner.send_keys(keys).await?;
        Ok(())
    }

    /// Whether the element is currently pressed, by `aria-pressed`.
    pub async fn pressed(&self) -> Result<bool> {
        Ok(self.prop("ariaPressed").await? == "true")
    }

    /// Whether the control can be worked at all.
    pub async fn enabled(&self) -> Result<bool> {
        Ok(self.inner.is_enabled().await?)
    }

    /// Whether the control says it is out of play, by `aria-disabled`. This is
    /// the question to ask of a custom control: `enabled` reads the form
    /// control's `disabled`, which an element that is not a form control
    /// cannot carry, so it answers `true` for one however dead it is.
    pub async fn disabled(&self) -> Result<bool> {
        Ok(self.prop("ariaDisabled").await? == "true")
    }

    common!();
}

/// Wrapper for `WebDriver` which adds convenience methods for testing.
pub struct TestDriver {
    inner: WebDriver,
    downloads: tempfile::TempDir,
    wait_timeout: Duration,
    load_timeout: Duration,
    waiting: Waiting,
}

impl TestDriver {
    /// **`epoch_ms` on the browser's own clock**: its month (from one), day,
    /// hour and minute, for a test reading a time the page shows in the
    /// reader's zone.
    pub async fn local_clock(&self, epoch_ms: u64) -> Result<[u32; 4]> {
        let read = self
            .inner
            .execute(
                "const d = new Date(arguments[0]); \
                 return [d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes()]",
                vec![serde_json::json!(epoch_ms)],
            )
            .await?;
        let parts = read.json().as_array().context("clock array")?;
        let mut clock = [0; 4];

        for (slot, part) in clock.iter_mut().zip(parts) {
            *slot = u32::try_from(part.as_u64().context("clock part")?)?;
        }

        Ok(clock)
    }

    /// **Every matching rendered text in one browser observation.**
    ///
    /// A live viewer can replace its rows while WebDriver walks a list of
    /// element handles. Read the list inside the document when the
    /// observation itself is the assertion, so it is one snapshot rather
    /// than a handle per row.
    pub async fn rendered_texts(&self, selector: &str) -> Result<Vec<String>> {
        let read = self
            .inner
            .execute(
                "return Array.from(document.querySelectorAll(arguments[0])) \
                 .map(n => n.innerText)",
                vec![serde_json::json!(selector)],
            )
            .await?;
        let rows = read.json().as_array().context("rendered text array")?;

        rows.iter()
            .map(|row| row.as_str().context("rendered text").map(str::to_owned))
            .collect()
    }

    /// Files saved by this browser, cleared before each test and removed when it closes.
    pub fn downloads(&self) -> &std::path::Path {
        self.downloads.path()
    }

    /// What this browser's test was last waiting for, for the runner to hang
    /// off a report of giving up on it.
    pub(crate) fn waiting(&self) -> Waiting {
        self.waiting.clone()
    }

    /// The WebDriver session underneath, for what this wrapper has no method
    /// for.
    pub fn webdriver(&self) -> &WebDriver {
        &self.inner
    }

    /// How long one observation of a page that is already up may take to
    /// agree: the deadline [`Self::wait_until`] and the finds wait against.
    pub fn wait_timeout(&self) -> Duration {
        self.wait_timeout
    }

    /// How long the first look at a page may take: the browser starting, the
    /// bundle being fetched, the wasm compiled, none of which has happened
    /// when a page is first asked for.
    pub fn load_timeout(&self) -> Duration {
        self.load_timeout
    }

    /// Resize the browser window.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<()> {
        tracing::trace!(width, height, "Sizing the window");
        self.inner.set_window_rect(0, 0, width, height).await?;
        Ok(())
    }

    /// **Force one inline CSS property on an element**, the same lever a
    /// hand-typed `style=""` pulls, for a size no real window is narrow
    /// enough to produce on its own, such as a popover's own row.
    pub async fn set_style(
        &self,
        element: &TestElement,
        property: &str,
        value: &str,
    ) -> Result<()> {
        tracing::trace!(property, value, "Setting an inline style property");

        let script = format!("arguments[0].style.setProperty({property:?}, {value:?});");

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// **Force one class on or off an element**, for a state the app has no
    /// way to put that element in yet but a stylesheet rule already answers
    /// for, such as a record button that is capturing and unavailable at once.
    pub async fn set_class(&self, element: &TestElement, class: &str, on: bool) -> Result<()> {
        tracing::trace!(class, on, "Forcing a class");

        let script = format!("arguments[0].classList.toggle({class:?}, {on});");

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// **Give a field a value the way a native control would**, then fire
    /// `event` on it, for fields such as `<input type="color">` whose own
    /// picker the driver cannot reach.
    pub async fn set_value(&self, element: &TestElement, value: &str, event: &str) -> Result<()> {
        tracing::trace!(value, event, "Setting a field's value");

        self.inner
            .execute(
                "arguments[0].value = arguments[1]; \
                 arguments[0].dispatchEvent(new Event(arguments[2], {bubbles: true}));",
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(value),
                    serde_json::json!(event),
                ],
            )
            .await?;

        Ok(())
    }

    /// **Replace the text an element already shows**, for content the test
    /// machine cannot be relied on to produce, such as a very long name. Only the existing text node's data changes, so the
    /// page keeps owning the node.
    pub async fn set_text(&self, element: &TestElement, text: &str) -> Result<()> {
        tracing::trace!(text, "Replacing an element's text");

        let script = format!(
            "const node = arguments[0].firstChild; \
             if (!node || node.nodeType !== Node.TEXT_NODE) throw new Error('no text node'); \
             node.data = {text:?};"
        );

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// **Every box inside an element that has more to show than it has room
    /// for**, one line each: a scroller with something to scroll, or a box
    /// that clips what it holds. Empty when everything in it fits. A box a
    /// pixel or less across is passed over, since that is how a control is
    /// hidden from sight while staying reachable, and it clips by design.
    pub async fn overflowing(&self, element: &TestElement) -> Result<Vec<String>> {
        tracing::trace!("Finding what overflows inside an element");

        let script = "const out = []; \
             for (const el of [arguments[0], ...arguments[0].querySelectorAll('*')]) { \
                 if (el.clientHeight <= 1 || el.clientWidth <= 1) continue; \
                 const cs = getComputedStyle(el); \
                 const y = el.scrollHeight - el.clientHeight; \
                 const x = el.scrollWidth - el.clientWidth; \
                 const shows = v => v === 'auto' || v === 'scroll' || v === 'hidden' || v === 'clip'; \
                 if ((y > 1 && shows(cs.overflowY)) || (x > 1 && shows(cs.overflowX))) { \
                     const test = el.getAttribute('data-test'); \
                     out.push(el.localName + (el.className ? '.' + String(el.className).trim().split(/\\s+/).join('.') : '') \
                         + (test ? `[data-test=${test}]` : '') \
                         + ` ${cs.overflowX}/${cs.overflowY} over by ${x}x${y}`); \
                 } \
             } \
             return out;";

        let found = self
            .inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(found
            .json()
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_owned))
            .collect())
    }

    /// **Every piece of text inside an element set smaller than `floor`
    /// pixels**, one line each, passing over whatever sits inside `marks`: the
    /// numbers of a scale or a badge's count, which are read at a glance rather
    /// than as words. Text nobody can see is passed over too.
    pub async fn small_text(
        &self,
        element: &TestElement,
        floor: f64,
        marks: &str,
    ) -> Result<Vec<String>> {
        tracing::trace!(floor, marks, "Finding text set below a size");

        let script = format!(
            "const out = []; \
             for (const el of [arguments[0], ...arguments[0].querySelectorAll('*')]) {{ \
                 if (el.closest({marks:?})) continue; \
                 const words = Array.from(el.childNodes) \
                     .filter(n => n.nodeType === Node.TEXT_NODE) \
                     .map(n => n.textContent.trim()).join(' ').trim(); \
                 if (!words) continue; \
                 const rect = el.getBoundingClientRect(); \
                 if (rect.width <= 1 || rect.height <= 1) continue; \
                 const cs = getComputedStyle(el); \
                 if (cs.visibility !== 'visible') continue; \
                 const size = parseFloat(cs.fontSize); \
                 if (size < {floor}) {{ \
                     const test = el.getAttribute('data-test'); \
                     out.push(el.localName + (el.className ? '.' + String(el.className).trim().split(/\\s+/).join('.') : '') \
                         + (test ? `[data-test=${{test}}]` : '') \
                         + ` at ${{cs.fontSize}}: ${{JSON.stringify(words.slice(0, 32))}}`); \
                 }} \
             }} \
             return out;"
        );

        let found = self
            .inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(found
            .json()
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_owned))
            .collect())
    }

    /// **Turn the wheel and report whether anything consumed it.**
    ///
    /// Dispatches the same event as [`Self::wheel`], and returns `true` when a
    /// listener called `preventDefault()` on it.
    pub async fn wheel_consumed(&self, element: &TestElement, delta: f64) -> Result<bool> {
        tracing::trace!(delta, "Turning the wheel and asking who took it");

        let script = format!(
            "return !arguments[0].dispatchEvent(new WheelEvent('wheel', \
             {{ deltaY: {delta}, bubbles: true, cancelable: true }}))"
        );

        let taken = self
            .inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(taken.json().as_bool().unwrap_or(false))
    }

    /// Turn the wheel over an element: a cancelable, bubbling `wheel` event
    /// with `deltaY` set to `delta` (positive scrolls down), dispatched on it.
    ///
    /// The event is synthesized in the page rather than sent through
    /// WebDriver, so it reaches the element's listeners but the browser does
    /// not scroll anything by itself. Returns once the event is dispatched;
    /// see [`Self::wheel_consumed`] to learn whether anything handled it.
    pub async fn wheel(&self, element: &TestElement, delta: f64) -> Result<()> {
        tracing::trace!(delta, "Turning the wheel over an element");

        let script = format!(
            "arguments[0].dispatchEvent(new WheelEvent('wheel', \
             {{ deltaY: {delta}, bubbles: true, cancelable: true }}))"
        );

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// **Turn the wheel with `alt` held**, and `shift` too where asked.
    ///
    /// The pointer is put at the middle of the element rather than left at the
    /// origin a fresh `WheelEvent` carries: this zoom holds still whatever is
    /// under the pointer, so a notch at `clientY` zero anchors on the top edge
    /// and walks what is shown straight out of the window.
    pub async fn wheel_alt(&self, element: &TestElement, delta: f64, shift: bool) -> Result<()> {
        tracing::trace!(delta, shift, "Turning the wheel with alt held");

        let script = format!(
            "const box = arguments[0].getBoundingClientRect(); \
             arguments[0].dispatchEvent(new WheelEvent('wheel', \
             {{ deltaY: {delta}, altKey: true, shiftKey: {shift}, \
             clientX: box.left + box.width / 2, \
             clientY: box.top + box.height / 2, \
             bubbles: true, cancelable: true }}))"
        );

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// **Turn the wheel over an element and say whether the page took it.**
    ///
    /// Both axes and `ctrl` are given, because that is what tells one turn from
    /// another where an element answers for more than one of them, and the
    /// pointer is put in the middle of the element so a zoom that holds still
    /// whatever is under it has something to hold.
    ///
    /// What comes back is whether the default was prevented, which is the only
    /// thing that separates a gesture the window answered from one it answered
    /// *and* let fall through to scroll the box it landed in.
    pub async fn wheel_over(
        &self,
        element: &TestElement,
        across: f64,
        down: f64,
        ctrl: bool,
    ) -> Result<bool> {
        tracing::trace!(across, down, ctrl, "Turning the wheel over both axes");

        let script = format!(
            "const box = arguments[0].getBoundingClientRect(); \
             return !arguments[0].dispatchEvent(new WheelEvent('wheel', \
             {{ deltaX: {across}, deltaY: {down}, ctrlKey: {ctrl}, \
             clientX: box.left + box.width / 2, \
             clientY: box.top + box.height / 2, \
             bubbles: true, cancelable: true }}))"
        );

        let taken = self
            .inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(taken.json().as_bool().unwrap_or(false))
    }

    /// **Turn the wheel sideways**, reading as `deltaX`: the gesture a trackpad
    /// makes to pan, and not the one `wheel` makes.
    pub async fn wheel_across(&self, element: &TestElement, delta: f64) -> Result<()> {
        tracing::trace!(delta, "Turning the wheel sideways over an element");

        let script = format!(
            "arguments[0].dispatchEvent(new WheelEvent('wheel', \
             {{ deltaX: {delta}, bubbles: true, cancelable: true }}))"
        );

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Capture clipboard writes in this page without accessing the system clipboard.
    ///
    /// Replaces `navigator.clipboard.writeText` so the text it is handed is
    /// kept on the page instead, where [`Self::wait_copied`] reads it. Lasts
    /// until the page is next loaded.
    pub async fn capture_clipboard(&self) -> Result<()> {
        self.inner
            .execute(
                "const inject = document.createElement('script'); \
                 inject.textContent = `navigator.clipboard.writeText = async text => { \
                     document.body.setAttribute('data-copied', text); \
                 };`; \
                 document.documentElement.appendChild(inject); inject.remove();",
                Vec::new(),
            )
            .await?;
        Ok(())
    }

    /// Delay every `WebSocket` send in this page by `millis`, so what the page
    /// shows while a request is in flight can be observed. Lasts until
    /// [`Self::stop_delaying_websocket_sends`] or the page is next loaded.
    ///
    /// This defers the call into the real `WebSocket.send`, which means it
    /// must not defer with the caller's own bytes: `send_with_u8_array` hands
    /// JS a raw view over wasm linear memory, not a copy, and musli-web's
    /// client reuses one scratch buffer for every request, clearing it the
    /// moment `send` returns. A connection something else keeps sending on
    /// (a poll on a timer, say) would otherwise overwrite
    /// that memory before a long enough delay's deferred call ever reads it,
    /// corrupting the frame it finally sends. `data.slice()` copies the
    /// bytes out synchronously, before this function returns control to the
    /// caller -- the same moment a real, un-shimmed `send()` would -- so the
    /// delay only ever holds a snapshot, the way a slow network holds bytes
    /// already handed off, never a live alias into memory the caller is free
    /// to keep mutating.
    pub async fn delay_websocket_sends(&self, millis: u64) -> Result<()> {
        self.inner.execute(format!(
            "const inject = document.createElement('script'); \
             inject.textContent = `(() => {{ \
                 WebSocket.__e2eOriginalSend = WebSocket.__e2eOriginalSend \
                     || WebSocket.prototype.send; \
                 const send = WebSocket.__e2eOriginalSend; \
                 WebSocket.prototype.send = function(data) {{ \
                     const copy = data && typeof data.slice === 'function' \
                         ? data.slice() : data; \
                     setTimeout(() => {{ if (this.readyState === 1) send.call(this, copy); }}, {millis}); \
                 }}; \
             }})();`; \
             document.documentElement.appendChild(inject); inject.remove();"
        ), Vec::new()).await?;
        Ok(())
    }

    /// Undo [`Self::delay_websocket_sends`], now that whatever it was
    /// installed to slow down has been observed. The patched `send` stays in
    /// effect for the rest of the page's life otherwise, taxing every later
    /// request on the same connection by the same delay -- fine for a test
    /// that stops looking right after, a trap for one that keeps going and
    /// pays it on every send from here to the next page load.
    pub async fn stop_delaying_websocket_sends(&self) -> Result<()> {
        self.inner
            .execute(
                "const inject = document.createElement('script'); \
                 inject.textContent = `(() => { \
                     if (WebSocket.__e2eOriginalSend) { \
                         WebSocket.prototype.send = WebSocket.__e2eOriginalSend; \
                         delete WebSocket.__e2eOriginalSend; \
                     } \
                 })();`; \
                 document.documentElement.appendChild(inject); inject.remove();"
                    .to_owned(),
                Vec::new(),
            )
            .await?;
        Ok(())
    }

    /// Wait until the text last written to the clipboard since
    /// [`Self::capture_clipboard`] is exactly `text`.
    pub async fn wait_copied(&self, text: &str) -> Result<()> {
        self.wait_until("the full text to be copied", async || {
            Ok(self.find_one_by("body").await?.attr("data-copied").await? == text)
        })
        .await
    }

    /// Start watching for anything the page throws.
    pub async fn watch_for_errors(&self) -> Result<()> {
        tracing::trace!("Listening for uncaught errors in the page");

        let script = "if (!document.body.hasAttribute('data-watching')) { \
                 document.body.setAttribute('data-watching', ''); \
                 const inject = document.createElement('script'); \
                 inject.textContent = `\
                     const say = what => document.body.setAttribute( \
                         'data-threw', String(what).slice(0, 6000)); \
                     window.addEventListener('error', e => say( \
                         (e.error && e.error.stack) || e.message)); \
                     window.addEventListener('unhandledrejection', e => say(e.reason)); \
                     const was = console.error; \
                     console.error = (...a) => { say(a.join(' ')); was(...a); }; \
                 `; \
                 document.documentElement.appendChild(inject); \
                 inject.remove(); \
             }";

        self.inner.execute(script, Vec::new()).await?;
        Ok(())
    }

    /// What a page that did not become what was asked for has to say for
    /// itself, as a sentence to hang off a timeout: whether anything is
    /// mounted, what `data-page` the body carries, what it threw, and how the
    /// document and its bundle loaded.
    pub async fn what_the_page_says(&self) -> Result<String> {
        let script = "const b = document.body; \
             if (!b) { return 'and the document has no body at all'; } \
             const page = b.getAttribute('data-page'); \
             const threw = b.getAttribute('data-threw'); \
             const mounted = b.children.length; \
             let out = page ? `; the page showing is '${page}'` \
                 : '; nothing has claimed the page yet'; \
             out += `, ${mounted} element(s) are mounted under the body`; \
             if (threw) { out += `, and the page threw: ${threw}`; } \
             out += `, the document is ${document.readyState}`; \
             out += `, the bundle's exports are ${typeof window.wasmBindings}`; \
             try { new WebAssembly.Module( \
                 new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])); \
                 out += ', an empty module still compiles'; } \
             catch (e) { out += ', and an empty module will not compile: ' + e; } \
             out += ', history=' + history.length; \
             const nav = performance.getEntriesByType('navigation')[0]; \
             if (nav) { out += `, the document was answered after \
                 ${Math.max(0, Math.round(nav.responseEnd))}ms`; } \
             for (const got of performance.getEntriesByType('resource')) { \
                 if (!/\\.(wasm|js)(\\?|$)/.test(got.name)) { continue; } \
                 out += `, ${got.name.split('/').pop()} was fetched in \
                     ${Math.round(got.duration)}ms`; } \
             return out;";

        let told = self.inner.execute(script, Vec::new()).await?;
        Ok(told.json().as_str().unwrap_or_default().to_owned())
    }

    /// What the page threw since [`Self::watch_for_errors`], if anything did.
    pub async fn error_seen(&self) -> Result<Option<String>> {
        let body = self.find_one_by("body").await?;
        let seen = body.attr("data-threw").await?;

        Ok((!seen.is_empty()).then_some(seen))
    }

    /// Carry one element onto another and let go.
    pub async fn pointer_drag_onto(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        from.inner.scroll_into_view().await?;
        self.inner
            .action_chain()
            .move_to_element_center(&from.inner)
            .click_and_hold()
            .move_by_offset(8, 0)
            .release_on_element(&to.inner)
            .perform()
            .await?;
        Ok(())
    }

    /// **Carry one element over another with the pointer, the way a hand
    /// does, and keep hold**: press on it, move off in small steps so the
    /// browser starts its own drag, and come to rest over the target. Unlike
    /// [`Self::drag_over`] nothing is dispatched, so whatever stops the browser
    /// starting a drag, or a target taking one, stops this too. Let go with
    /// [`Self::native_release`].
    pub async fn native_drag_over(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        tracing::trace!("Carrying one element over another with the pointer");

        from.inner.scroll_into_view().await?;

        self.inner
            .action_chain()
            .move_to_element_center(&from.inner)
            .click_and_hold()
            .move_by_offset(4, 0)
            .move_by_offset(8, 2)
            .move_by_offset(16, 4)
            .move_to_element_center(&to.inner)
            .move_by_offset(2, 0)
            .perform()
            .await?;

        Ok(())
    }

    /// Let go of every key and button the actions above hold, the WebDriver
    /// way, which also puts down a drag an engine would otherwise keep
    /// carrying into the next test.
    pub async fn release_all_input(&self) -> Result<()> {
        self.inner.action_chain().reset_actions().await?;
        Ok(())
    }

    /// Let go of what [`Self::native_drag_over`] is holding, where it is.
    pub async fn native_release(&self) -> Result<()> {
        self.inner
            .action_chain()
            .move_by_offset(1, 0)
            .release()
            .perform()
            .await?;
        Ok(())
    }

    /// **Start counting what changes in the document under `element`**: every
    /// node added or removed, attribute set and text changed there, and the
    /// element itself taken out of the page (moving it does that), and every
    /// canvas in it cleared to be drawn again, read back with
    /// [`Self::mutations`]. Watching again starts the count over.
    pub async fn watch_mutations(&self, element: &TestElement) -> Result<()> {
        self.inner
            .execute(
                "if (window.__e2eMutations) { \
                   window.__e2eMutations.observer.disconnect(); \
                   window.__e2eMutations.outer.disconnect(); \
                 } \
                 const watch = { count: 0, kinds: [] }; \
                 watch.observer = new MutationObserver(records => { \
                   watch.count += records.length; \
                   for (const r of records) watch.kinds.push(r.type + ':' + (r.attributeName || '') + ':' + (r.target.className || r.target.nodeName)); \
                 }); \
                 watch.observer.observe(arguments[0], \
                   { subtree: true, childList: true, attributes: true, characterData: true }); \
                 const node = arguments[0]; \
                 watch.outer = new MutationObserver(records => { \
                   for (const r of records) for (const n of r.removedNodes) \
                     if (n === node || n.contains(node)) { watch.count += 1; watch.kinds.push('removed'); } \
                 }); \
                 watch.outer.observe(document.body, { subtree: true, childList: true }); \
                 if (!window.__e2eClearRect) { \
                   window.__e2eClearRect = CanvasRenderingContext2D.prototype.clearRect; \
                   CanvasRenderingContext2D.prototype.clearRect = function (...args) { \
                     const watch = window.__e2eMutations; \
                     if (watch && watch.node.contains(this.canvas)) { watch.count += 1; watch.kinds.push('redrawn'); } \
                     return window.__e2eClearRect.apply(this, args); \
                   }; \
                 } \
                 watch.node = node; \
                 window.__e2eMutations = watch;",
                vec![element.inner.to_json()?],
            )
            .await?;
        Ok(())
    }

    /// How many changes [`Self::watch_mutations`] has seen, and what each was:
    /// its kind, the attribute where it was one, and the class of the node.
    pub async fn mutations(&self) -> Result<(u64, Vec<String>)> {
        let value = self
            .inner
            .execute(
                "const watch = window.__e2eMutations; \
                 return watch ? [watch.count, watch.kinds] : [0, []];",
                Vec::new(),
            )
            .await?;
        Ok(serde_json::from_value(value.json().clone())?)
    }

    /// Load real local files into a browser FileList without starting an import.
    pub async fn external_files(&self, paths: &str) -> Result<TestElement> {
        self.inner.execute("const input = document.createElement('input'); input.type = 'file'; input.multiple = true; input.hidden = true; input.dataset.test = 'external-files'; document.body.appendChild(input);", Vec::new()).await?;
        let input = self.find_one_by("[data-test='external-files']").await?;
        input.send_keys(paths).await?;
        Ok(input)
    }

    /// Dispatch the browser file payload onto a drop target; report cancellation.
    pub async fn drag_external_files(
        &self,
        files: &TestElement,
        to: &TestElement,
        kind: &str,
    ) -> Result<bool> {
        let result = self.inner.execute("const data = new DataTransfer(); for (const file of arguments[0].files) data.items.add(file); const event = new DragEvent(arguments[2], {dataTransfer: data, bubbles: true, cancelable: true}); arguments[1].dispatchEvent(event); return event.defaultPrevented;", vec![files.inner.to_json()?, to.inner.to_json()?, kind.into()]).await?;
        Ok(serde_json::from_value(result.json().clone())?)
    }

    /// Carry one element onto another through its HTML drag handlers.
    pub async fn drag_onto(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        tracing::trace!("Carrying one element onto another");

        let script = "const data = new DataTransfer(); \
             const fire = (node, kind) => node.dispatchEvent( \
                 new DragEvent(kind, \
                     { dataTransfer: data, bubbles: true, cancelable: true })); \
             fire(arguments[0], 'dragstart'); \
             fire(arguments[1], 'dragover'); \
             fire(arguments[1], 'drop'); \
             fire(arguments[0], 'dragend');";

        self.inner
            .execute(script, vec![from.inner.to_json()?, to.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Carry one element over another without letting go, so that what the
    /// controller says about the offer can be read.
    pub async fn drag_over(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        tracing::trace!("Carrying one element over another");

        let script = "const data = new DataTransfer(); \
             const fire = (node, kind) => node.dispatchEvent( \
                 new DragEvent(kind, \
                     { dataTransfer: data, bubbles: true, cancelable: true })); \
             fire(arguments[0], 'dragstart'); \
             fire(arguments[1], 'dragover');";

        self.inner
            .execute(script, vec![from.inner.to_json()?, to.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Carry a drag in progress back off the element it was over, which is the
    /// `dragleave` a browser sends and neither [`Self::drag_over`] nor
    /// [`Self::drag_onto`] does: both begin their own drag, so this is handed
    /// the element the carry is leaving and nothing else.
    pub async fn drag_off(&self, left: &TestElement) -> Result<()> {
        tracing::trace!("Carrying a drag back off an element");

        let script = "const data = new DataTransfer(); \
             arguments[0].dispatchEvent( \
                 new DragEvent('dragleave', \
                     { dataTransfer: data, bubbles: true, cancelable: true }));";

        self.inner
            .execute(script, vec![left.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Set a container's vertical scroll, letting the browser emit its scroll event.
    pub async fn scroll_top(&self, element: &TestElement, top: i32) -> Result<()> {
        self.inner
            .execute(
                "arguments[0].scrollTop = arguments[1];",
                vec![element.inner.to_json()?, serde_json::json!(top)],
            )
            .await?;
        Ok(())
    }

    /// Pin whatever scrolls this element sideways to the far end of its travel.
    pub async fn scroll_to_far_end(&self, element: &TestElement) -> Result<()> {
        tracing::trace!("Scrolling an element's box to the end of its travel");

        let script = "let node = arguments[0].parentElement; \
             while (node) { \
                 const how = getComputedStyle(node).overflowX; \
                 if ((how === 'auto' || how === 'scroll') \
                     && node.scrollWidth > node.clientWidth) { \
                     break; \
                 } \
                 node = node.parentElement; \
             } \
             if (node) { node.scrollLeft = node.scrollWidth; }";

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// **What the browser settled on, rather than what a rule says.**
    pub async fn computed(
        &self,
        element: &TestElement,
        pseudo: &str,
        property: &str,
    ) -> Result<String> {
        tracing::trace!(pseudo, property, "Asking what a property computed to");

        let script = "return getComputedStyle(arguments[0], arguments[1] || null) \
             .getPropertyValue(arguments[2]);";

        let value = self
            .inner
            .execute(
                script,
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(pseudo),
                    serde_json::json!(property),
                ],
            )
            .await?;

        Ok(value.json().as_str().unwrap_or_default().trim().to_string())
    }

    /// Whether the page matches `prefers-reduced-motion: reduce`.
    ///
    /// Where `E2E_MOTION` is set this is an error when the browser disagrees
    /// with it, so a run meant to exercise one preference cannot quietly get
    /// the other. Set it (Firefox only) to test the browser's own media
    /// preference rather than a replacement stylesheet.
    pub async fn reduced_motion(&self) -> Result<bool> {
        let value = self
            .inner
            .execute(
                "return matchMedia('(prefers-reduced-motion: reduce)').matches;",
                Vec::new(),
            )
            .await?;
        let reduced = value
            .json()
            .as_bool()
            .context("reduced-motion media query")?;
        if let Ok(requested) = std::env::var("E2E_MOTION") {
            anyhow::ensure!(
                reduced == (requested == "reduce"),
                "browser did not honor E2E_MOTION={requested}"
            );
        }
        Ok(reduced)
    }

    /// Pause an actual CSS animation at a deterministic phase before reading its
    /// computed presentation. Missing animations are failures, not idle samples.
    pub async fn animation_time(
        &self,
        element: &TestElement,
        name: &str,
        millis: f64,
    ) -> Result<()> {
        self.inner.execute(
            "const animation = arguments[0].getAnimations().find(a => a.animationName === arguments[1]); \
             if (!animation) throw new Error('Missing animation: ' + arguments[1]); \
             animation.pause(); animation.currentTime = arguments[2];",
            vec![element.inner.to_json()?, serde_json::json!(name), serde_json::json!(millis)],
        ).await?;
        Ok(())
    }

    /// Read a computed color's alpha through the browser's color parser so both
    /// legacy rgba() and CSS Color 4 serialization describe the same measurement.
    pub async fn color_alpha(&self, element: &TestElement, property: &str) -> Result<f64> {
        let value = self.inner.execute(
            "const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1; \
             const context = canvas.getContext('2d'); context.fillStyle = 'transparent'; \
             context.fillStyle = getComputedStyle(arguments[0]).getPropertyValue(arguments[1]); \
             context.fillRect(0, 0, 1, 1); return context.getImageData(0, 0, 1, 1).data[3] / 255;",
            vec![element.inner.to_json()?, serde_json::json!(property)],
        ).await?;
        value.json().as_f64().context("computed color alpha")
    }

    /// **Which way a computed colour moves what is painted under it.** The
    /// property is filled over mid grey by the browser itself and the result
    /// compared with it, so a band's direction is measured rather than read off
    /// a serialization: negative darkens, positive lightens, zero paints
    /// nothing. `color-mix()` and a legacy `rgba()` are the same measurement.
    pub async fn color_shift(&self, element: &TestElement, property: &str) -> Result<f64> {
        let [red, green, blue] = self.painted(element, property).await?;
        Ok((red + green + blue) / 3.0 - 128.0)
    }

    /// **What a computed colour actually paints**, as red, green and blue.
    ///
    /// The browser fills the property over mid grey and the pixel is read back,
    /// so a `color-mix()`, a legacy `rgba()` and a translucent wash all answer
    /// as the colour that is seen rather than as whatever the property happens
    /// to serialize to.
    pub async fn painted(&self, element: &TestElement, property: &str) -> Result<[f64; 3]> {
        let value = self.inner.execute(
            "const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1; \
             const context = canvas.getContext('2d'); \
             context.fillStyle = '#808080'; context.fillRect(0, 0, 1, 1); \
             context.fillStyle = 'transparent'; \
             context.fillStyle = getComputedStyle(arguments[0]).getPropertyValue(arguments[1]); \
             context.fillRect(0, 0, 1, 1); \
             const painted = context.getImageData(0, 0, 1, 1).data; \
             return [painted[0], painted[1], painted[2]];",
            vec![element.inner.to_json()?, serde_json::json!(property)],
        ).await?;

        let read = value.json();
        let read = read.as_array().context("what a computed colour paints")?;

        let mut rgb = [0.0; 3];

        for (channel, value) in rgb.iter_mut().zip(read) {
            *channel = value.as_f64().context("a painted channel")?;
        }

        Ok(rgb)
    }

    /// **A canvas's own drawn envelope**, read from its actual pixel buffer
    /// rather than any DOM or SVG geometry: a `<canvas>` carries no
    /// `points`/`viewBox` attribute a test could read instead. Samples up to `columns` evenly
    /// spaced points across the canvas's backing-buffer width; each entry is
    /// `(x, top, bottom)`, all fractions (0.0..=1.0) of the canvas's own box
    /// -- `x` across its width, `top`/`bottom` the first and last drawn
    /// (non-transparent) pixel row down its height. A sampled column with
    /// nothing drawn reads `(x, -1.0, -1.0)`.
    pub async fn canvas_columns(
        &self,
        canvas: &TestElement,
        columns: usize,
    ) -> Result<Vec<(f64, f64, f64)>> {
        let script = "const canvas = arguments[0]; \
             const columns = Math.max(1, arguments[1]); \
             const w = canvas.width, h = canvas.height; \
             if (w === 0 || h === 0) return []; \
             const ctx = canvas.getContext('2d'); \
             const data = ctx.getImageData(0, 0, w, h).data; \
             const step = Math.max(1, Math.floor(w / columns)); \
             const out = []; \
             for (let x = 0; x < w; x += step) { \
                 let top = -1, bottom = -1; \
                 for (let y = 0; y < h; y++) { \
                     if (data[(y * w + x) * 4 + 3] > 0) { \
                         if (top === -1) top = y; \
                         bottom = y; \
                     } \
                 } \
                 out.push([x / w, top === -1 ? -1 : top / h, bottom === -1 ? -1 : bottom / h]); \
             } \
             return out;";

        let value = self
            .inner
            .execute(
                script,
                vec![canvas.inner.to_json()?, serde_json::json!(columns)],
            )
            .await?;

        let read = value.json();
        let read = read.as_array().context("canvas column scan")?;
        let mut out = Vec::with_capacity(read.len());

        for row in read {
            let row = row.as_array().context("a canvas column")?;
            let field = |at: usize, name: &str| {
                row.get(at)
                    .and_then(serde_json::Value::as_f64)
                    .with_context(|| format!("a canvas column's {name}"))
            };
            out.push((field(0, "x")?, field(1, "top")?, field(2, "bottom")?));
        }

        Ok(out)
    }

    /// A canvas's backing-buffer size in device pixels alongside its
    /// own CSS box and the browser's reported device pixel ratio, so a caller
    /// can check the three agree -- that the buffer is sized for the actual
    /// visible box and pixel density, not a fixed or stale guess.
    pub async fn canvas_size(&self, canvas: &TestElement) -> Result<CanvasSize> {
        let script = "const c = arguments[0]; const r = c.getBoundingClientRect(); \
             return [c.width, c.height, r.width, r.height, window.devicePixelRatio];";

        let value = self
            .inner
            .execute(script, vec![canvas.inner.to_json()?])
            .await?;
        let read = value.json();
        let read = read.as_array().context("canvas size read")?;
        let field = |at: usize, name: &str| {
            read.get(at)
                .and_then(serde_json::Value::as_f64)
                .with_context(|| format!("canvas size's {name}"))
        };

        Ok(CanvasSize {
            device_width: field(0, "device_width")?,
            device_height: field(1, "device_height")?,
            css_width: field(2, "css_width")?,
            css_height: field(3, "css_height")?,
            device_pixel_ratio: field(4, "device_pixel_ratio")?,
        })
    }

    /// Press the middle button over an element.
    pub async fn middle_click(&self, element: &TestElement) -> Result<()> {
        tracing::trace!("Middle-clicking an element");

        let script = "for (const type of ['mousedown', 'mouseup', 'auxclick']) {\
             arguments[0].dispatchEvent(new MouseEvent(type, \
             { button: 1, buttons: 4, bubbles: true, cancelable: true })); \
         }";

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Pan with a native middle-button drag, releasing the button at the end.
    pub async fn middle_drag_by(&self, element: &TestElement, dx: i64) -> Result<()> {
        use thirtyfour::common::command::Command;

        let actions = serde_json::json!([{
            "type": "pointer", "id": "pointer", "parameters": {"pointerType": "mouse"},
            "actions": [
                {"type": "pointerMove", "duration": 0, "origin": element.inner.to_json()?, "x": 0, "y": 0},
                {"type": "pointerDown", "button": 1},
                {"type": "pointerMove", "duration": 150, "origin": "pointer", "x": dx, "y": 0},
                {"type": "pointerUp", "button": 1}
            ]
        }]);
        self.inner
            .cmd(Command::PerformActions(actions.into()))
            .await?;
        Ok(())
    }

    /// Carry a held drag somewhere else without letting go of it.
    pub async fn move_held(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Carrying a held drag");

        self.inner
            .action_chain()
            .move_by_offset(dx, dy)
            .perform()
            .await?;

        Ok(())
    }

    /// Let go of whatever [`TestElement::drag_by`] is holding.
    pub async fn drop_held(&self) -> Result<()> {
        tracing::trace!("Releasing a held drag");
        self.inner.action_chain().release().perform().await?;
        Ok(())
    }

    /// Cancel a captured pointer without releasing the test's held button, so
    /// a later mouse-up can prove that cancelled work is not committed.
    pub async fn cancel_pointer(&self, element: &TestElement) -> Result<()> {
        self.inner
            .execute(
                "arguments[0].dispatchEvent(new PointerEvent('pointercancel', { bubbles: true }));",
                vec![element.inner.to_json()?],
            )
            .await?;
        Ok(())
    }

    /// The same for a drag that was begun with `alt` held, which has to give
    /// the key back as well as the button.
    pub async fn drop_held_alt(&self) -> Result<()> {
        tracing::trace!("Releasing a held drag and the alt key with it");

        self.inner
            .action_chain()
            .release()
            .key_up(thirtyfour::Key::Alt)
            .perform()
            .await?;

        Ok(())
    }

    /// Whether what is drawn at one point of the page belongs to `selector` and
    /// nothing is standing over it. A frame says what is drawn somewhere but
    /// not what drew it, so a test measuring a window's own content has to be
    /// able to tell that content from whatever floats in front of it.
    ///
    /// Two questions, because one does not do. Hit testing says which element
    /// is uppermost at the point, which settles the ordinary case. But a menu,
    /// a drag ghost or a help bubble is drawn through a portal into the
    /// document body, over the top of a window and often wider than it, and a
    /// bubble takes no pointer events at all -- hit testing looks straight
    /// through it and answers with the window underneath. So anything portaled
    /// beside the application is asked separately whether it covers the point.
    ///
    /// The point is in the page's own pixels, not the frame's.
    pub async fn drawn_inside(&self, selector: &str, x: f64, y: f64) -> Result<bool> {
        tracing::trace!(selector, x, y, "Asking what is drawn at a point");

        let script = "const [selector, x, y] = arguments; \
             const host = document.querySelector(selector); \
             const found = document.elementFromPoint(x, y); \
             if (!host || !found || !host.contains(found)) return false; \
             for (const over of document.body.children) { \
                 if (over.contains(host) || host.contains(over)) continue; \
                 const box = over.getBoundingClientRect(); \
                 if (x >= box.left && x <= box.right && y >= box.top && y <= box.bottom) \
                     return false; \
             } \
             return true;";

        let value = self
            .inner
            .execute(
                script,
                vec![
                    serde_json::json!(selector),
                    serde_json::json!(x),
                    serde_json::json!(y),
                ],
            )
            .await?;

        Ok(value.json().as_bool().unwrap_or_default())
    }

    /// A screenshot of the whole page as the PNG the browser hands back, for a
    /// test that reads a frame rather than writing one out. Taken whether or
    /// not `E2E_SHOTS` is set, since an assertion that measures a frame has to
    /// have one on every run rather than only when somebody is looking.
    pub async fn screenshot_png(&self) -> Result<Vec<u8>> {
        tracing::trace!("Taking a screenshot");
        Ok(self.inner.screenshot_as_png().await?)
    }

    /// Write a screenshot of the whole page to `path`.
    pub async fn screenshot(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        std::fs::write(path, self.screenshot_png().await?)?;
        Ok(())
    }

    /// The same, into `$E2E_SHOTS/<name>.png`, and nothing at all when that is
    /// unset. Left in a test rather than added and removed, since it costs nothing
    /// on an ordinary run.
    ///
    /// **`E2E_SHOTS` is a directory, and it reads like a flag.** A relative one
    /// is taken under `target/e2e-shots`, where everything else the suite leaves
    /// behind goes — so `E2E_SHOTS=1`, which is what a hand types when it means
    /// *yes*, writes to `target/e2e-shots/1` rather than making a directory
    /// called `1` in whatever the test process's working directory happens to
    /// be, which under cargo is the package, in the checkout.
    ///
    /// ```text
    /// E2E_SHOTS=1 cargo test -p app-e2e -- cart::
    /// E2E_SHOTS=/tmp/shots cargo test -p app-e2e -- cart::
    /// ```
    pub async fn snapshot(&self, name: &str) -> Result<()> {
        let Some(dir) = std::env::var_os("E2E_SHOTS") else {
            return Ok(());
        };

        let dir = std::path::PathBuf::from(dir);

        let dir = match dir.is_absolute() {
            true => dir,
            false => crate::dist::target_dir()?.join("e2e-shots").join(dir),
        };

        std::fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{name}.png"));
        tracing::info!(path = %path.display(), "Writing snapshot");
        self.screenshot(&path).await
    }

    /// Press the `index`th element matching `by`, re-finding it if the page
    /// moves underneath.
    pub async fn press_nth(&self, by: impl IntoBy, index: usize) -> Result<()> {
        let by = by.into_by();

        tracing::trace!(?by, index, "Pressing element");

        let deadline = time::Instant::now() + self.wait_timeout;

        loop {
            let found = self.find_all(by.clone()).await?;

            if let Some(item) = found.get(index)
                && item.click().await.is_ok()
            {
                return Ok(());
            }

            if time::Instant::now() >= deadline {
                bail!(
                    "Timed out after {:?} pressing {by} at {index}",
                    self.wait_timeout
                );
            }

            time::sleep(POLL_INTERVAL).await;
        }
    }

    /// The same as [`TestDriver::press_nth`], right-handed. A context menu that
    /// belongs to a thing is opened on the thing itself, so there is no button
    /// to press and this is how a test reaches one.
    pub async fn context_press_nth(&self, by: impl IntoBy, index: usize) -> Result<()> {
        let by = by.into_by();

        tracing::trace!(?by, index, "Right-clicking element");

        let deadline = time::Instant::now() + self.wait_timeout;

        loop {
            let found = self.find_all(by.clone()).await?;

            if let Some(item) = found.get(index)
                && item.context_click().await.is_ok()
            {
                return Ok(());
            }

            if time::Instant::now() >= deadline {
                bail!(
                    "Timed out after {:?} right-clicking {by} at {index}",
                    self.wait_timeout
                );
            }

            time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Read a browser-persisted setting without changing it.
    pub async fn local_storage(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .inner
            .execute(
                "return localStorage.getItem(arguments[0]);",
                vec![serde_json::json!(key)],
            )
            .await?;
        Ok(value.json().as_str().map(str::to_owned))
    }

    /// Seed a persisted setting before mounting the component that reads it.
    pub async fn set_local_storage(&self, key: &str, value: &str) -> Result<()> {
        self.inner
            .execute(
                "localStorage.setItem(arguments[0], arguments[1]);",
                vec![serde_json::json!(key), serde_json::json!(value)],
            )
            .await?;
        Ok(())
    }

    /// Deny access for the current document, as when browser policy blocks storage.
    pub async fn block_local_storage(&self) -> Result<()> {
        self.inner.execute(
            "const script = document.createElement('script'); \
             script.textContent = `Object.defineProperty(window, 'localStorage', {configurable: true, get() { throw new DOMException('Storage blocked by test', 'SecurityError'); }});`; \
             document.documentElement.appendChild(script); script.remove();",
            vec![],
        ).await?;
        Ok(())
    }

    /// Reload the page, which is the only way to ask what the *server* holds.
    pub async fn reload(&self) -> Result<()> {
        tracing::trace!("Reloading the page");
        self.inner.refresh().await?;

        let _ = self.watch_for_errors().await;
        Ok(())
    }

    /// Open the page again with `query` in the address.
    pub async fn reopen_with(&self, query: &str) -> Result<()> {
        tracing::trace!(query, "Reopening the page");

        let mut url = self.inner.current_url().await?;
        url.set_query(Some(query));
        self.inner.goto(url.as_str()).await?;
        Ok(())
    }

    /// Run `run` in a second tab showing the page this one shows, then close
    /// it and come back. The tab is sized as every test's first one is, and
    /// `run` is what waits for the page to come up; what the page threw fails
    /// it as it would the test.
    pub async fn in_second_tab(&self, run: impl AsyncFnOnce() -> Result<()>) -> Result<()> {
        self.in_second_context(true, run).await
    }

    /// The same in a second window.
    pub async fn in_second_window(&self, run: impl AsyncFnOnce() -> Result<()>) -> Result<()> {
        self.in_second_context(false, run).await
    }

    async fn in_second_context(
        &self,
        tab: bool,
        run: impl AsyncFnOnce() -> Result<()>,
    ) -> Result<()> {
        let original = self.inner.window().await?;
        let url = self.inner.current_url().await?;
        let opened = if tab {
            self.inner.new_tab().await?
        } else {
            self.inner.new_window().await?
        };

        let result = async {
            self.inner.switch_to_window(opened.clone()).await?;
            self.set_window_size(WINDOW.0, WINDOW.1).await?;
            self.inner.goto(url.as_str()).await?;
            run().await?;
            match self.error_seen().await? {
                Some(threw) => bail!("the page threw: {threw}"),
                None => Ok(()),
            }
        }
        .await;

        let cleanup = async {
            self.inner.switch_to_window(opened).await?;
            self.inner.close_window().await?;
            self.inner.switch_to_window(original).await?;
            Ok(())
        }
        .await;

        match result {
            Ok(()) => cleanup,
            Err(error) => Err(with_cleanup(error, cleanup)),
        }
    }

    /// Press a key with nothing in particular focused, the way a hand does when
    /// it has just opened something with the pointer.
    pub async fn press_key(&self, key: Key) -> Result<()> {
        tracing::trace!(?key, "Pressing a key");
        self.inner.action_chain().send_keys(key).perform().await?;
        Ok(())
    }

    /// Hold a real WebDriver key across observations, rather than sending a
    /// down/up pair too quickly for an animation frame to show it.
    pub async fn hold_key(&self, key: char) -> Result<()> {
        self.inner.action_chain().key_down(key).perform().await?;
        Ok(())
    }

    /// Let go of a key held with [`Self::hold_key`].
    pub async fn release_key(&self, key: char) -> Result<()> {
        self.inner.action_chain().key_up(key).perform().await?;
        Ok(())
    }

    /// **Every match's attribute, read in one pass.** Reading them one handle
    /// at a time is how a test goes stale against a window that redraws on the
    /// animation frame: the elements it is holding are replaced between two reads
    /// and the second one throws.
    pub async fn find_all_attrs(&self, selector: &str, attr: &str) -> Result<Vec<String>> {
        let script = format!(
            "return Array.from(document.querySelectorAll({})) \
             .map(n => n.getAttribute({}) ?? '')",
            serde_json::to_string(selector)?,
            serde_json::to_string(attr)?,
        );

        let read = self.inner.execute(script, Vec::new()).await?;

        Ok(read
            .json()
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|row| row.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default())
    }

    /// **Where every match sits down the page, with the attribute that names
    /// it, in one pass.** A band is a top and a height, which is all two things
    /// drawn in different columns have to agree about.
    ///
    /// Measuring a hundred elements one handle at a time is a round trip each
    /// for the attribute and another for the box, which is a test that runs out
    /// of time rather than one that fails. A node the browser is not drawing has
    /// no box and is left out, so a count is also an answer to what is shown.
    pub async fn bands(&self, selector: &str, attr: &str) -> Result<Vec<(String, f64, f64)>> {
        let script = format!(
            "return Array.from(document.querySelectorAll({})) \
             .filter(n => n.getClientRects().length > 0) \
             .map(n => [n.getAttribute({}) ?? '', \
             n.getBoundingClientRect().top, n.getBoundingClientRect().height])",
            serde_json::to_string(selector)?,
            serde_json::to_string(attr)?,
        );

        let read = self.inner.execute(script, Vec::new()).await?;
        let read = read.json();

        let rows = read.as_array().context("a list of bands")?;
        let mut bands = Vec::with_capacity(rows.len());

        for row in rows {
            let row = row.as_array().context("a band")?;

            bands.push((
                row.first()
                    .and_then(|name| name.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                row.get(1).and_then(|top| top.as_f64()).context("a top")?,
                row.get(2)
                    .and_then(|height| height.as_f64())
                    .context("a height")?,
            ));
        }

        Ok(bands)
    }

    /// Dispatch a named key on a focused element, including keys WebDriver cannot encode.
    pub async fn press_key_on(&self, selector: &str, key: &str) -> Result<()> {
        let script = format!(
            "const at = document.querySelector({}); \
             if (!at) throw new Error('No element for keyboard press'); \
             at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             {{ key: {}, bubbles: true, cancelable: true }}));",
            serde_json::to_string(selector)?,
            serde_json::to_string(key)?,
        );
        self.inner.execute(script, Vec::new()).await?;
        Ok(())
    }

    /// **Press a key on one element with `control` held**, which is how undo,
    /// redo and select-all are reached.
    ///
    /// Dispatched on the element so the press lands where the test says. The
    /// event carries the `code` a real letter key does, since a listener on
    /// the document may read the code and a press without one would never
    /// reach it.
    pub async fn press_key_with_ctrl(&self, selector: &str, key: &str) -> Result<()> {
        tracing::trace!(selector, key, "Pressing a key with control held");

        let code = match key.chars().collect::<Vec<_>>().as_slice() {
            [c] if c.is_ascii_alphabetic() => format!("Key{}", c.to_ascii_uppercase()),
            _ => key.to_owned(),
        };

        let script = format!(
            "const at = document.querySelector({}); \
             if (at) {{ at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             {{ key: {}, code: {}, ctrlKey: true, bubbles: true, cancelable: true }})); }}",
            serde_json::to_string(selector)?,
            serde_json::to_string(key)?,
            serde_json::to_string(&code)?,
        );

        self.inner.execute(script, Vec::new()).await?;
        Ok(())
    }

    /// **Press a key on one element with shift held**, dispatched for the
    /// same reason [`Self::press_key_with_ctrl`] is.
    pub async fn press_key_with_shift(&self, selector: &str, key: &str) -> Result<()> {
        tracing::trace!(selector, key, "Pressing a key with shift held");

        let script = format!(
            "const at = document.querySelector({}); \
             if (at) {{ at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             {{ key: {}, shiftKey: true, bubbles: true, cancelable: true }})); }}",
            serde_json::to_string(selector)?,
            serde_json::to_string(key)?,
        );

        self.inner.execute(script, Vec::new()).await?;
        Ok(())
    }

    /// Take keyboard focus off whatever has it, so the next key lands on the
    /// page rather than on a control.
    pub async fn blur(&self) -> Result<()> {
        tracing::trace!("Blurring the focused element");

        self.inner
            .execute(
                "document.activeElement && document.activeElement.blur();",
                Vec::new(),
            )
            .await?;

        Ok(())
    }

    /// One attribute of whatever element has keyboard focus, or `None` when
    /// it has no such attribute. Where nothing is focused, that is the
    /// document's `<body>`.
    ///
    /// Answers once, at the time of asking; it does not wait.
    pub async fn focused_attr(&self, name: &str) -> Result<Option<String>> {
        let found = self
            .inner
            .execute(
                &format!(
                    "return document.activeElement \
                     && document.activeElement.getAttribute('{name}')"
                ),
                Vec::new(),
            )
            .await?;

        Ok(found.json().as_str().map(str::to_owned))
    }

    common!();
}

/// **Anything that names elements to find**, taken by every lookup on
/// [`TestDriver`] that says `by`.
///
/// A string (`&str` or `&String`) is a CSS selector; any other way of finding
/// elements is written out as a [`By`].
///
/// # Examples
///
/// ```no_run
/// use yew_e2e::prelude::*;
///
/// async fn lookups(driver: &TestDriver) -> Result<()> {
///     // A CSS selector.
///     driver.wait_count("li.item", 3).await?;
///
///     // One that was built rather than written out.
///     let id = 7;
///     driver.find_one_by(&format!("[data-test=row-{id}]")).await?;
///
///     // Any other `By`.
///     driver.find_first(By::XPath("//button[text()='Save']")).await?;
///     Ok(())
/// }
/// ```
pub trait IntoBy {
    /// The [`By`] this names.
    fn into_by(self) -> By;
}

impl IntoBy for &str {
    #[inline]
    fn into_by(self) -> By {
        By::Css(self)
    }
}

/// A selector that was built rather than written out, such as one naming a
/// control whose `data-test` carries an identifier in it.
impl IntoBy for &String {
    #[inline]
    fn into_by(self) -> By {
        By::Css(self.as_str())
    }
}

impl IntoBy for By {
    #[inline]
    fn into_by(self) -> By {
        self
    }
}
