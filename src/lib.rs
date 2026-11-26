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
//! * The frontend built once per run with `trunk build` or `trug build`, or served
//!   from `E2E_DIST` when it is already built.
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
//! `Setup`. One preceded by `#[ignore]`, or `#[ignore = "reason"]`, runs only
//! under `--include-ignored`:
//!
//! ```ignore
//! yew_e2e::harness! {
//!     App;
//!     cart::{adds_an_item, removes_an_item},
//!     login::{remembers_the_user(signed_in)},
//!     flaky::{
//!         #[ignore = "waits on the upstream fix"]
//!         races_the_server,
//!     },
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
//! the workspace root, [`Frontend::Trug(dir)`] does the same with `trug build`
//! for an app built with [trug], and [`Frontend::None`] builds nothing, for a
//! fixture that serves its own pages. `E2E_DIST` names a frontend already built,
//! to serve in place of building one.
//!
//! Everything else a run leaves behind goes under the build directory too: the
//! record of every run in `target/e2e-sessions`, the run lock, a fetched
//! `chromedriver` in `target/e2e`, named for the Chrome build it matches so that
//! an updated Chrome fetches a new one. Two suites sharing one build directory give
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
//! | `--include-ignored` | Run the tests declared `#[ignore]` as well as the rest. |
//! | `--last-session`, `--session NAME` | Run only what did not pass in that session. |
//! | `--timeout SECONDS` | How long one test may run. Defaults to [`Config::timeout`], else 30; 0 waits for ever. |
//! | `--grace SECONDS` | How long a running test is given to finish after Ctrl-C or SIGTERM. |
//! | `--lock target\|project\|none` | Which other runs this one waits for: those in the same build directory (the default), every worktree of the Git project, or none. |
//! | `--lock-wait SECONDS` | How long to wait for that lock. |
//!
//! | Variable | |
//! |---|---|
//! | `E2E_HEADED` | As `--headed`. |
//! | `E2E_BROWSER` | As `--browser`. |
//! | `E2E_INCLUDE_IGNORED` | As `--include-ignored`. |
//! | `E2E_DIST` | A frontend that is already built, served in place of building one. |
//! | `E2E_THEME` | `dark` or `light`, for the colour scheme the browser reports (Firefox). |
//! | `E2E_MOTION` | `reduce` or `normal`, for the motion preference the browser reports (Firefox). |
//! | `E2E_SCALE` | The device pixel ratio the browser reports. |
//! | `E2E_SHOTS` | Where [`TestDriver::snapshot`] writes; relative to `target/e2e-shots`. |
//!
//! [`Config::lock_name`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.lock_name
//! [`Config::sessions_dir`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.sessions_dir
//! [`Config::timeout`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.timeout
//! [`dist()`]: https://docs.rs/yew-e2e/latest/yew_e2e/fn.dist.html
//! [`examples/static_page.rs`]: https://github.com/udoprog/yew-e2e/blob/main/examples/static_page.rs
//! [`Fixture`]: https://docs.rs/yew-e2e/latest/yew_e2e/trait.Fixture.html
//! [`Frontend::None`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.None
//! [`Frontend::Trunk(dir)`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.Trunk
//! [`Frontend::Trug(dir)`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.Trug
//! [`harness!`]: https://docs.rs/yew-e2e/latest/yew_e2e/macro.harness.html
//! [`TestDriver::snapshot`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.TestDriver.html#method.snapshot
//! [`TestDriver`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.TestDriver.html
//! [Trunk]: https://trunkrs.dev
//! [trug]: https://github.com/udoprog/trug
//! [Yew]: https://yew.rs

#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::future::Future;
use std::path::PathBuf;

use anyhow::Result;

mod browser;
mod dist;
mod driver;
mod element;
mod fetch;
mod finding;
pub mod harness;
#[cfg(test)]
mod lib_tests;
mod run_lock;
mod sessions;
mod test_driver;

pub use self::dist::{dist, target_dir, workspace_root};
pub use self::driver::Engine;
pub use self::element::TestElement;
pub use self::finding::{IntoBy, POLL_INTERVAL};
pub use self::test_driver::{CanvasSize, TestDriver};

pub(crate) use self::browser::{Browser, Holding, WINDOW, Waiting, overran, run_one};

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
    pub(crate) timeout: Option<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            about: String::from("A browser suite."),
            frontend: Frontend::Trunk(PathBuf::new()),
            lock: String::from("yew-e2e.lock"),
            sessions: String::from("e2e-sessions"),
            timeout: None,
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

    /// How long one test may run, in seconds, where `--timeout` is not given;
    /// zero waits for ever. Without this it is 30.
    pub fn timeout(mut self, seconds: u16) -> Self {
        self.timeout = Some(seconds);
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
    /// `trug build` run in this directory, relative to the workspace root,
    /// into `e2e-dist` under the build directory. `E2E_DIST` names a frontend
    /// already built in its place.
    Trug(PathBuf),
}
