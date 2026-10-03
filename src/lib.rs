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

use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow, bail, ensure};

use thirtyfour::prelude::*;
use tokio::time::{self, Duration};

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

pub use self::dist::{dist, target_dir, workspace_root};
pub use self::driver::Engine;
pub use self::element::TestElement;
pub use self::finding::{IntoBy, POLL_INTERVAL};

pub(crate) use self::browser::{Browser, Holding, WINDOW, Waiting, overran, run_one};
use self::driver::with_cleanup;
use self::finding::common;

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

/// A number handed to a page script as an argument, refused unless it is
/// finite: JSON has no infinity or NaN, so the page would read `null` instead.
fn finite(what: &str, value: f64) -> Result<serde_json::Value> {
    ensure!(value.is_finite(), "{what} must be finite, not {value}");
    Ok(serde_json::json!(value))
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

/// Wrapper for `WebDriver` which adds convenience methods for testing.
pub struct TestDriver {
    inner: WebDriver,
    downloads: tempfile::TempDir,
    wait_timeout: Duration,
    load_timeout: Duration,
    waiting: Waiting,
    /// Whether this test asked for [`TestDriver::watch_for_errors`], so the
    /// helpers that navigate know to watch the page they arrive at too.
    watching: AtomicBool,
    holding: Holding,
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

        self.inner
            .execute(
                "arguments[0].style.setProperty(arguments[1], arguments[2]);",
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(property),
                    serde_json::json!(value),
                ],
            )
            .await?;

        Ok(())
    }

    /// **Force one class on or off an element**, for a state the app has no
    /// way to put that element in yet but a stylesheet rule already answers
    /// for, such as a record button that is capturing and unavailable at once.
    pub async fn set_class(&self, element: &TestElement, class: &str, on: bool) -> Result<()> {
        tracing::trace!(class, on, "Forcing a class");

        self.inner
            .execute(
                "arguments[0].classList.toggle(arguments[1], arguments[2]);",
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(class),
                    serde_json::json!(on),
                ],
            )
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

        let script = "const node = arguments[0].firstChild; \
             if (!node || node.nodeType !== Node.TEXT_NODE) throw new Error('no text node'); \
             node.data = arguments[1];";

        self.inner
            .execute(
                script,
                vec![element.inner.to_json()?, serde_json::json!(text)],
            )
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

        let floor = finite("a text size floor", floor)?;

        let script = "const out = []; \
             for (const el of [arguments[0], ...arguments[0].querySelectorAll('*')]) { \
                 if (el.closest(arguments[1])) continue; \
                 const words = Array.from(el.childNodes) \
                     .filter(n => n.nodeType === Node.TEXT_NODE) \
                     .map(n => n.textContent.trim()).join(' ').trim(); \
                 if (!words) continue; \
                 const rect = el.getBoundingClientRect(); \
                 if (rect.width <= 1 || rect.height <= 1) continue; \
                 const cs = getComputedStyle(el); \
                 if (cs.visibility !== 'visible') continue; \
                 const size = parseFloat(cs.fontSize); \
                 if (size < arguments[2]) { \
                     const test = el.getAttribute('data-test'); \
                     out.push(el.localName + (el.className ? '.' + String(el.className).trim().split(/\\s+/).join('.') : '') \
                         + (test ? `[data-test=${test}]` : '') \
                         + ` at ${cs.fontSize}: ${JSON.stringify(words.slice(0, 32))}`); \
                 } \
             } \
             return out;";

        let found = self
            .inner
            .execute(
                script,
                vec![element.inner.to_json()?, serde_json::json!(marks), floor],
            )
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

        let delta = finite("a wheel delta", delta)?;

        let taken = self
            .inner
            .execute(
                "return !arguments[0].dispatchEvent(new WheelEvent('wheel', \
                 { deltaY: arguments[1], bubbles: true, cancelable: true }))",
                vec![element.inner.to_json()?, delta],
            )
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

        let delta = finite("a wheel delta", delta)?;

        self.inner
            .execute(
                "arguments[0].dispatchEvent(new WheelEvent('wheel', \
                 { deltaY: arguments[1], bubbles: true, cancelable: true }))",
                vec![element.inner.to_json()?, delta],
            )
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

        let delta = finite("a wheel delta", delta)?;

        let script = "const box = arguments[0].getBoundingClientRect(); \
             arguments[0].dispatchEvent(new WheelEvent('wheel', \
             { deltaY: arguments[1], altKey: true, shiftKey: arguments[2], \
             clientX: box.left + box.width / 2, \
             clientY: box.top + box.height / 2, \
             bubbles: true, cancelable: true }))";

        self.inner
            .execute(
                script,
                vec![element.inner.to_json()?, delta, serde_json::json!(shift)],
            )
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

        let across = finite("a sideways wheel delta", across)?;
        let down = finite("a wheel delta", down)?;

        let script = "const box = arguments[0].getBoundingClientRect(); \
             return !arguments[0].dispatchEvent(new WheelEvent('wheel', \
             { deltaX: arguments[1], deltaY: arguments[2], ctrlKey: arguments[3], \
             clientX: box.left + box.width / 2, \
             clientY: box.top + box.height / 2, \
             bubbles: true, cancelable: true }))";

        let taken = self
            .inner
            .execute(
                script,
                vec![
                    element.inner.to_json()?,
                    across,
                    down,
                    serde_json::json!(ctrl),
                ],
            )
            .await?;

        Ok(taken.json().as_bool().unwrap_or(false))
    }

    /// **Turn the wheel sideways**, reading as `deltaX`: the gesture a trackpad
    /// makes to pan, and not the one `wheel` makes.
    pub async fn wheel_across(&self, element: &TestElement, delta: f64) -> Result<()> {
        tracing::trace!(delta, "Turning the wheel sideways over an element");

        let delta = finite("a sideways wheel delta", delta)?;

        self.inner
            .execute(
                "arguments[0].dispatchEvent(new WheelEvent('wheel', \
                 { deltaX: arguments[1], bubbles: true, cancelable: true }))",
                vec![element.inner.to_json()?, delta],
            )
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
        // The injected source is built in the page, from the delay as a
        // number, since the script element cannot read `arguments`.
        let script = "const inject = document.createElement('script'); \
             inject.textContent = `(() => { \
                 WebSocket.__e2eOriginalSend = WebSocket.__e2eOriginalSend \
                     || WebSocket.prototype.send; \
                 const send = WebSocket.__e2eOriginalSend; \
                 WebSocket.prototype.send = function(data) { \
                     const copy = data && typeof data.slice === 'function' \
                         ? data.slice() : data; \
                     setTimeout(() => { if (this.readyState === 1) send.call(this, copy); }, \
                         ${JSON.stringify(arguments[0])}); \
                 }; \
             })();`; \
             document.documentElement.appendChild(inject); inject.remove();";

        self.inner
            .execute(script, vec![serde_json::json!(millis)])
            .await?;
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
    ///
    /// The listener is installed by a script that keeps its names to itself,
    /// so it neither collides with the page's globals nor claims any the page
    /// declares later. It marks the body `data-watching` once it is in place,
    /// and this fails if that mark does not appear, since a watch that never
    /// started would let every throw pass unheard.
    ///
    /// What it records for a thrown error is its message followed by its
    /// stack, since some browsers (Firefox) keep only the frames in the stack.
    ///
    /// The watch is the test's from here on: [`Self::reload`],
    /// [`Self::reopen_with`], [`Self::in_second_tab`] and
    /// [`Self::in_second_window`] start it again on the page they arrive at,
    /// and a test that never asked for it is left unwatched by them.
    pub async fn watch_for_errors(&self) -> Result<()> {
        self.watching.store(true, Ordering::Relaxed);
        self.install_watch().await
    }

    /// Watch the page this has just navigated to, if the test is watching.
    async fn rewatch(&self) -> Result<()> {
        if !self.watching.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.install_watch()
            .await
            .context("watching the page again after navigating")
    }

    async fn install_watch(&self) -> Result<()> {
        tracing::trace!("Listening for uncaught errors in the page");

        let script = "if (!document.body.hasAttribute('data-watching')) { \
                 const inject = document.createElement('script'); \
                 inject.textContent = `(() => { \
                     const say = what => document.body.setAttribute( \
                         'data-threw', String(what).slice(0, 6000)); \
                     const told = (thrown, otherwise) => { \
                         if (!(thrown instanceof Error)) { return otherwise; } \
                         const head = String(thrown); \
                         const stack = thrown.stack || ''; \
                         return stack.startsWith(head) ? stack : head + '\\\\n' + stack; \
                     }; \
                     window.addEventListener('error', e => say(told(e.error, e.message))); \
                     window.addEventListener('unhandledrejection', \
                         e => say(told(e.reason, e.reason))); \
                     const was = console.error; \
                     console.error = (...a) => { say(a.join(' ')); was(...a); }; \
                     document.body.setAttribute('data-watching', ''); \
                 })();`; \
                 document.documentElement.appendChild(inject); \
                 inject.remove(); \
             } \
             return document.body.hasAttribute('data-watching');";

        let watching = self.inner.execute(script, Vec::new()).await?;

        ensure!(
            watching.json().as_bool() == Some(true),
            "the script that watches the page for errors did not run"
        );
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
        self.holding.set(true);

        Ok(())
    }

    /// Let go of every key and button the actions above hold, the WebDriver
    /// way, which also puts down a drag an engine would otherwise keep
    /// carrying into the next test.
    ///
    /// A drag begun here is let go where the pointer is, as the specification
    /// asks; chromedriver's own release would let it go where it was pressed.
    pub async fn release_all_input(&self) -> Result<()> {
        if self.holding.take() {
            self.inner.action_chain().release().perform().await?;
        }

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
        self.holding.set(false);
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
        self.holding.set(false);
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
        self.holding.set(false);

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
    ///
    /// Giving up says why the last press did not land, such as another
    /// element covering this one, under the timeout.
    pub async fn press_nth(&self, by: impl IntoBy, index: usize) -> Result<()> {
        let by = by.into_by();

        tracing::trace!(?by, index, "Pressing element");

        let deadline = time::Instant::now() + self.wait_timeout;
        let mut refused = None;

        loop {
            let found = self.find_all(by.clone()).await?;

            if let Some(item) = found.get(index) {
                match item.click().await {
                    Ok(()) => return Ok(()),
                    Err(error) => refused = Some(error),
                }
            }

            if time::Instant::now() >= deadline {
                let timeout = format!(
                    "Timed out after {:?} pressing {by} at {index}",
                    self.wait_timeout
                );

                return Err(match refused {
                    Some(error) => error.context(timeout),
                    None => anyhow!(timeout),
                });
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
        let mut refused = None;

        loop {
            let found = self.find_all(by.clone()).await?;

            if let Some(item) = found.get(index) {
                match item.context_click().await {
                    Ok(()) => return Ok(()),
                    Err(error) => refused = Some(error),
                }
            }

            if time::Instant::now() >= deadline {
                let timeout = format!(
                    "Timed out after {:?} right-clicking {by} at {index}",
                    self.wait_timeout
                );

                return Err(match refused {
                    Some(error) => error.context(timeout),
                    None => anyhow!(timeout),
                });
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
    /// A test watching for errors goes on watching the reloaded page.
    pub async fn reload(&self) -> Result<()> {
        tracing::trace!("Reloading the page");
        self.inner.refresh().await?;
        self.rewatch().await
    }

    /// Open the page again with `query` in the address. A test watching for
    /// errors goes on watching the reopened page.
    pub async fn reopen_with(&self, query: &str) -> Result<()> {
        tracing::trace!(query, "Reopening the page");

        let mut url = self.inner.current_url().await?;
        url.set_query(Some(query));
        self.inner.goto(url.as_str()).await?;
        self.rewatch().await
    }

    /// Run `run` in a second tab showing the page this one shows, then close
    /// it and come back. The tab is sized as every test's first one is, and
    /// `run` is what waits for the page to come up. A test watching for errors
    /// goes on watching in the second tab, and what the page there threw fails
    /// it as it would the test; a test that never asked for the watch is left
    /// unwatched there too.
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
            self.rewatch().await?;
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
        let read = self
            .inner
            .execute(
                "return Array.from(document.querySelectorAll(arguments[0])) \
                 .map(n => n.getAttribute(arguments[1]) ?? '')",
                vec![serde_json::json!(selector), serde_json::json!(attr)],
            )
            .await?;

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
        let script = "return Array.from(document.querySelectorAll(arguments[0])) \
             .filter(n => n.getClientRects().length > 0) \
             .map(n => [n.getAttribute(arguments[1]) ?? '', \
             n.getBoundingClientRect().top, n.getBoundingClientRect().height])";

        let read = self
            .inner
            .execute(
                script,
                vec![serde_json::json!(selector), serde_json::json!(attr)],
            )
            .await?;
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
        let script = "const at = document.querySelector(arguments[0]); \
             if (!at) throw new Error('No element for keyboard press'); \
             at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             { key: arguments[1], bubbles: true, cancelable: true }));";
        self.inner
            .execute(
                script,
                vec![serde_json::json!(selector), serde_json::json!(key)],
            )
            .await?;
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

        let script = "const at = document.querySelector(arguments[0]); \
             if (at) { at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             { key: arguments[1], code: arguments[2], ctrlKey: true, bubbles: true, cancelable: true })); }";

        self.inner
            .execute(
                script,
                vec![
                    serde_json::json!(selector),
                    serde_json::json!(key),
                    serde_json::json!(code),
                ],
            )
            .await?;
        Ok(())
    }

    /// **Press a key on one element with shift held**, dispatched for the
    /// same reason [`Self::press_key_with_ctrl`] is.
    pub async fn press_key_with_shift(&self, selector: &str, key: &str) -> Result<()> {
        tracing::trace!(selector, key, "Pressing a key with shift held");

        let script = "const at = document.querySelector(arguments[0]); \
             if (at) { at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             { key: arguments[1], shiftKey: true, bubbles: true, cancelable: true })); }";

        self.inner
            .execute(
                script,
                vec![serde_json::json!(selector), serde_json::json!(key)],
            )
            .await?;
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
                "return document.activeElement \
                 && document.activeElement.getAttribute(arguments[0])",
                vec![serde_json::json!(name)],
            )
            .await?;

        Ok(found.json().as_str().map(str::to_owned))
    }

    common!();
}
