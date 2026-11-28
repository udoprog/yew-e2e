//! The runner's side of one browser: starting it, pointing it at each test's
//! fixture in turn, running the test against `--timeout`, and telling a test
//! that failed from a browser that went away.

use std::fmt::Write;
use std::panic::AssertUnwindSafe;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::FutureExt;
use thirtyfour::prelude::*;
use tokio::time::{self, Duration};

use crate::driver::{self, Driver, QUIT_TIMEOUT, STARTUP_TIMEOUT, with_cleanup};
use crate::finding::{LOAD_TIMEOUT, WAIT_TIMEOUT};
use crate::timings::{Phase, phase};
use crate::{Engine, Fixture, TestDriver};

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
    pub(crate) fn on(&self, what: impl std::fmt::Display) {
        *self.held() = Some(what.to_string());
    }

    /// Forget it, which is what the start of a test does.
    pub(crate) fn clear(&self) {
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

/// **Whether a drag begun here still has the main button down**, shared by a
/// driver and every element found through it, so that
/// [`TestDriver::release_all_input`] knows whether there is a button to let go
/// of where the pointer is. Chromedriver dispatches a `pointerup` for a button
/// nobody pressed, so releasing one blindly is not the no-op the WebDriver
/// specification makes it.
#[derive(Clone, Default)]
pub(crate) struct Holding(Arc<AtomicBool>);

impl Holding {
    pub(crate) fn set(&self, held: bool) {
        self.0.store(held, Ordering::Relaxed);
    }

    /// Whether the button was down, putting it up.
    pub(crate) fn take(&self) -> bool {
        self.0.swap(false, Ordering::Relaxed)
    }
}

/// One browser, kept for as long as there are tests to run in it.
pub(crate) struct Browser {
    pub(crate) webdriver: Driver,
    pub(crate) driver: TestDriver,
}

impl Browser {
    /// Start a WebDriver server and the one browser it drives.
    pub(crate) async fn open() -> Result<Self> {
        let engine = crate::harness::engine();
        let deadline = time::Instant::now() + STARTUP_TIMEOUT;
        let caps = engine.capabilities(crate::harness::headed())?;
        let webdriver = phase(Phase::DriverSpawn, Driver::new(engine, deadline)).await?;
        phase(
            Phase::SessionCreate,
            Self::connect(webdriver, caps, deadline),
        )
        .await
    }

    pub(crate) async fn connect(
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
                watching: AtomicBool::new(false),
                holding: Holding::default(),
                downloads,
            },
        })
    }

    /// Clear per-test runner state held on the driver and its download directory.
    fn clear_state(&self) -> Result<()> {
        self.driver.waiting.clear();
        self.driver.watching.store(false, Ordering::Relaxed);
        // A drag the last test left held is not this test's to let go of where
        // the pointer is: its release_all_input must not send a pointerup into
        // a page that never saw the press.
        self.driver.holding.set(false);
        for file in std::fs::read_dir(self.driver.downloads.path())? {
            std::fs::remove_file(file?.path())?;
        }
        Ok(())
    }

    /// Put the browser on a fresh tab, let `fixture` prepare it, then go to
    /// the fixture's page.
    ///
    /// The preceding document is closed rather than navigated away from. A
    /// WebDriver navigation leaves session history behind; long runs showed
    /// memory growth that closing the previous tab removed.
    async fn reset<F: Fixture>(&mut self, fixture: &mut F) -> Result<()> {
        self.clear_state()?;
        phase(Phase::TabReset, async {
            let old = self.driver.inner.window().await?;
            let fresh = self.driver.inner.new_tab().await?;
            self.driver.inner.switch_to_window(fresh.clone()).await?;
            self.driver.inner.switch_to_window(old).await?;
            self.driver.inner.close_window().await?;
            self.driver.inner.switch_to_window(fresh).await?;
            self.driver.set_window_size(WINDOW.0, WINDOW.1).await
        })
        .await?;
        phase(Phase::FixturePrepare, fixture.prepare(&mut self.driver)).await?;
        let url = fixture.url();
        phase(Phase::Goto, self.driver.inner.goto(&url)).await?;
        Ok(())
    }

    /// Clear per-test runner state on a browser whose fixture was recycled,
    /// keeping the current tab and document.
    async fn reset_recycled(&mut self) -> Result<()> {
        self.clear_state()?;
        phase(Phase::TabReset, async {
            self.driver.inner.enter_default_frame().await?;
            self.driver.set_window_size(WINDOW.0, WINDOW.1).await
        })
        .await
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

/// The window every test starts with; see [`Browser::reset`].
pub(crate) const WINDOW: (u32, u32) = (1600, 1000);

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

/// What [`run_one`] hands back to its worker: the test's result, the fixture
/// kept for the next test when it passed, and any non-fatal warnings raised
/// while recycling or quitting the preceding fixture.
pub(crate) struct RunOutcome<F> {
    pub(crate) result: Result<()>,
    pub(crate) fixture: Option<F>,
    pub(crate) warnings: Vec<anyhow::Error>,
}

/// Start (or recycle) the fixture for one test, run it, and either keep the
/// fixture on pass or take it down on failure.
pub(crate) async fn run_one<F: Fixture>(
    browser: &mut Browser,
    kept: Option<F>,
    start: crate::harness::Start<F>,
    setup: F::Setup,
    limit: Option<Duration>,
) -> RunOutcome<F> {
    let mut warnings = Vec::new();
    let mut recycled_fixture = None;

    if let Some(mut prev) = kept {
        browser.driver.waiting.clear();
        match phase(
            Phase::FixtureRecycle,
            prev.recycle(&mut browser.driver, &setup),
        )
        .await
        {
            Ok(true) => {
                recycled_fixture = Some(prev);
            }
            Ok(false) => {
                if let Err(error) = phase(Phase::FixtureQuit, prev.quit()).await {
                    warnings.push(error.context("quitting the previous fixture"));
                }
            }
            Err(error) => {
                if let Some(mark) = browser_death(&error) {
                    let fixture_result = phase(Phase::FixtureQuit, prev.quit()).await;
                    let mut error = went_away(browser, mark);

                    if let Err(quit) = fixture_result {
                        error = error.context(format!("the fixture also failed to quit: {quit}"));
                    }

                    return RunOutcome {
                        result: Err(error),
                        fixture: None,
                        warnings,
                    };
                }

                warnings.push(error.context("recycling the previous fixture"));

                if let Err(error) = phase(Phase::FixtureQuit, prev.quit()).await {
                    warnings.push(error.context("quitting the previous fixture"));
                }
            }
        }
    }

    let (mut fixture, recycled) = match recycled_fixture {
        Some(fixture) => (fixture, true),
        None => match phase(Phase::FixtureStart, F::start(setup)).await {
            Ok(fixture) => (fixture, false),
            Err(error) => {
                return RunOutcome {
                    result: Err(error),
                    fixture: None,
                    warnings,
                };
            }
        },
    };

    let prepared = if recycled {
        browser.reset_recycled().await
    } else {
        browser.reset(&mut fixture).await
    };

    let result = match prepared {
        Ok(()) => phase(Phase::Body, drive_one(browser, &mut fixture, start, limit)).await,
        Err(error) => Err(error),
    };

    let Err(error) = result else {
        return RunOutcome {
            result: Ok(()),
            fixture: Some(fixture),
            warnings,
        };
    };

    let fixture_result = phase(Phase::FixtureQuit, fixture.quit()).await;

    if let Some(mark) = browser_death(&error) {
        let mut error = went_away(browser, mark);

        if let Err(quit) = fixture_result {
            error = error.context(format!("the fixture also failed to quit: {quit}"));
        }

        return RunOutcome {
            result: Err(error),
            fixture: None,
            warnings,
        };
    }

    let mut errors = String::new();
    let _ = write_chain(&mut errors, "Test error", &error);

    if let Err(quit_error) = fixture_result {
        let _ = write_chain(&mut errors, "Fixture error", &quit_error);
    }

    RunOutcome {
        result: Err(anyhow!("{errors}")),
        fixture: None,
        warnings,
    }
}

/// One failure of a test's report: `label` and what the error says, then every
/// cause under it, one to a line.
fn write_chain(errors: &mut String, label: &str, error: &anyhow::Error) -> std::fmt::Result {
    writeln!(errors, "{label}: {error}")?;

    for cause in error.chain().skip(1) {
        writeln!(errors, "  Caused by: {cause}")?;
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
        Ok(Ok(Ok(()))) => driver.ensure_no_errors().await,
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

#[cfg(test)]
mod browser_tests {
    use anyhow::{Context, Result, ensure};

    use super::{Browser, write_chain};

    /// A failure reads as its label and message, then each cause on a line
    /// of its own.
    #[test]
    fn a_report_names_every_cause() -> Result<()> {
        let error = anyhow::anyhow!("the root")
            .context("the middle")
            .context("the top");

        let mut errors = String::new();
        write_chain(&mut errors, "Test error", &error)?;

        assert_eq!(
            errors,
            "Test error: the top\n  Caused by: the middle\n  Caused by: the root\n"
        );
        Ok(())
    }

    /// Each test gets a document with no predecessor retained in its browser.
    ///
    /// This uses the real WebDriver because only a browser can say which tabs
    /// and history entries survived. Replacing `Browser::reset` with a plain
    /// `goto` makes the second URL retain the first tab and add a history entry.
    #[tokio::test]
    #[ignore = "requires a real WebDriver"]
    async fn a_reset_discards_the_preceding_document() -> Result<()> {
        struct Page(&'static str);

        impl crate::Fixture for Page {
            type Setup = ();

            async fn start((): ()) -> Result<Self> {
                anyhow::bail!("not started by the runner")
            }

            fn url(&self) -> String {
                self.0.to_owned()
            }

            async fn quit(self) -> Result<()> {
                Ok(())
            }
        }

        let mut browser = Browser::open().await?;

        let result = async {
            browser.reset(&mut Page("data:text/html,first")).await?;
            let first = browser.driver.inner.window().await?;

            browser.reset(&mut Page("data:text/html,second")).await?;
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

    /// `Fixture::prepare` runs on the fresh tab before the first navigation,
    /// so a cookie it sets is sent with the fixture's first page load; a
    /// recycled fixture is not prepared again.
    #[tokio::test]
    #[ignore = "requires a real WebDriver"]
    async fn a_prepared_cookie_reaches_the_first_page_load() -> Result<()> {
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;

        use thirtyfour::Cookie;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        use tokio::task::JoinHandle;

        use crate::timings::{Phase, Timings, within};
        use crate::{Fixture, TestDriver};

        #[derive(Clone, Default)]
        struct CookieSetup {
            prepares: Rc<Cell<usize>>,
        }

        struct CookieFixture {
            port: u16,
            prepares: Rc<Cell<usize>>,
            server: JoinHandle<()>,
        }

        impl Fixture for CookieFixture {
            type Setup = CookieSetup;

            /// Serves every path a page naming the `Cookie` header its request
            /// came with.
            async fn start(setup: Self::Setup) -> Result<Self> {
                let listener = TcpListener::bind("127.0.0.1:0").await?;
                let port = listener.local_addr()?.port();

                let server = tokio::spawn(async move {
                    while let Ok((mut stream, _)) = listener.accept().await {
                        let mut request = [0; 8192];
                        let Ok(n) = stream.read(&mut request).await else {
                            continue;
                        };
                        let request = String::from_utf8_lossy(&request[..n]);
                        let cookie = request
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("cookie").then(|| value.trim())
                            })
                            .unwrap_or("");
                        let body = format!("<p id='cookie'>{cookie}</p>");
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\n\
                             content-length: {}\r\ncache-control: no-store\r\n\
                             connection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                });

                Ok(Self {
                    port,
                    prepares: setup.prepares,
                    server,
                })
            }

            fn url(&self) -> String {
                format!("http://127.0.0.1:{}/page", self.port)
            }

            async fn prepare(&mut self, driver: &mut TestDriver) -> Result<()> {
                self.prepares.set(self.prepares.get() + 1);
                driver
                    .webdriver()
                    .goto(format!("http://127.0.0.1:{}/blank", self.port))
                    .await?;
                driver
                    .webdriver()
                    .add_cookie(Cookie::new("session", "prepared"))
                    .await?;
                Ok(())
            }

            async fn recycle(
                &mut self,
                _driver: &mut TestDriver,
                _next: &Self::Setup,
            ) -> Result<bool> {
                Ok(true)
            }

            async fn quit(self) -> Result<()> {
                self.server.abort();
                Ok(())
            }
        }

        async fn sent_cookie(driver: &TestDriver) -> Result<String> {
            driver
                .inner
                .execute(
                    "return document.getElementById('cookie').textContent;",
                    Vec::new(),
                )
                .await?
                .json()
                .as_str()
                .map(str::to_owned)
                .context("the page's cookie text")
        }

        let mut browser = Browser::open().await?;
        let prepares = Rc::new(Cell::new(0));

        let result = async {
            let setup = || CookieSetup {
                prepares: prepares.clone(),
            };

            // 1. Fresh start: the first load of the fixture's page carries the cookie.
            let timings = Rc::new(RefCell::new(Timings::default()));
            let first = within(
                Some(&timings),
                super::run_one::<CookieFixture>(
                    &mut browser,
                    None,
                    |driver, _| {
                        Box::pin(async move {
                            let cookie = sent_cookie(driver).await?;
                            ensure!(
                                cookie == "session=prepared",
                                "the first load sent {cookie:?}"
                            );
                            Ok(())
                        })
                    },
                    setup(),
                    None,
                ),
            )
            .await;
            first.result?;
            let kept = first.fixture.context("passing test should keep fixture")?;
            ensure!(prepares.get() == 1, "prepared {} times", prepares.get());

            let phases = timings
                .borrow()
                .phases
                .iter()
                .map(|(phase, _)| *phase)
                .collect::<Vec<_>>();
            ensure!(
                phases
                    == [
                        Phase::FixtureStart,
                        Phase::TabReset,
                        Phase::FixturePrepare,
                        Phase::Goto,
                        Phase::Body
                    ],
                "unexpected fresh phases: {phases:?}"
            );

            // 2. Recycled: the page is kept and the fixture is not prepared again.
            let timings = Rc::new(RefCell::new(Timings::default()));
            let second = within(
                Some(&timings),
                super::run_one::<CookieFixture>(
                    &mut browser,
                    Some(kept),
                    |_, _| Box::pin(async { Ok(()) }),
                    setup(),
                    None,
                ),
            )
            .await;
            second.result?;
            ensure!(prepares.get() == 1, "a recycled fixture was prepared");
            let phases = timings
                .borrow()
                .phases
                .iter()
                .map(|(phase, _)| *phase)
                .collect::<Vec<_>>();
            ensure!(
                phases == [Phase::FixtureRecycle, Phase::TabReset, Phase::Body],
                "unexpected recycled phases: {phases:?}"
            );

            if let Some(fixture) = second.fixture {
                fixture.quit().await?;
            }

            Ok(())
        }
        .await;

        match (result, browser.close().await) {
            (Ok(()), Ok(_)) => Ok(()),
            (Err(error), Ok(_)) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => Err(super::driver::with_cleanup(error, Err(cleanup))),
        }
    }

    /// A recycled fixture keeps the same tab and document across tests while
    /// still clearing per-test runner state, falling back to a fresh fixture on
    /// `Ok(false)` or `Err`, and quitting immediately on test failure.
    #[tokio::test]
    #[ignore = "requires a real WebDriver"]
    async fn a_recycled_fixture_keeps_its_page_and_clears_per_test_state() -> Result<()> {
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        use std::sync::atomic::Ordering;

        use crate::timings::{Phase, Timings, within};
        use crate::{Fixture, TestDriver, WINDOW};

        #[derive(Clone, Copy, Default)]
        enum Mode {
            #[default]
            Keep,
            Decline,
            Fail,
        }

        #[derive(Clone, Default)]
        struct RecycleSetup {
            mode: Mode,
            starts: Rc<Cell<usize>>,
            quits: Rc<Cell<usize>>,
        }

        struct RecycleFixture {
            quits: Rc<Cell<usize>>,
        }

        impl Fixture for RecycleFixture {
            type Setup = RecycleSetup;

            async fn start(setup: Self::Setup) -> Result<Self> {
                setup.starts.set(setup.starts.get() + 1);
                Ok(Self { quits: setup.quits })
            }

            fn url(&self) -> String {
                "data:text/html,<iframe id='child' srcdoc='<p>inside</p>'></iframe>".to_owned()
            }

            async fn recycle(
                &mut self,
                _driver: &mut TestDriver,
                next: &Self::Setup,
            ) -> Result<bool> {
                match next.mode {
                    Mode::Keep => Ok(true),
                    Mode::Decline => Ok(false),
                    Mode::Fail => anyhow::bail!("recycle-failed-marker"),
                }
            }

            async fn quit(self) -> Result<()> {
                self.quits.set(self.quits.get() + 1);
                Ok(())
            }
        }

        let mut browser = Browser::open().await?;
        let starts = Rc::new(Cell::new(0));
        let quits = Rc::new(Cell::new(0));

        let result = async {
            let setup = |mode| RecycleSetup {
                mode,
                starts: starts.clone(),
                quits: quits.clone(),
            };

            // 1. Fresh start: dirty every piece of per-test state and enter an iframe.
            let first = super::run_one::<RecycleFixture>(
                &mut browser,
                None,
                |driver, _| {
                    Box::pin(async move {
                        driver
                            .inner
                            .execute("window.__recycled_marker = 'kept';", Vec::new())
                            .await?;
                        driver.set_window_size(900, 700).await?;
                        std::fs::write(driver.downloads().join("leftover.txt"), b"old")?;
                        driver.watch_for_errors().await?;
                        driver.holding.set(true);
                        driver.waiting.on("stale-wait");
                        let frame = driver.find_one_by("#child").await?;
                        frame.inner.enter_frame().await?;
                        Ok(())
                    })
                },
                setup(Mode::Keep),
                None,
            )
            .await;
            first.result?;
            ensure!(first.warnings.is_empty(), "unexpected warnings");
            let kept = first.fixture.context("passing test should keep fixture")?;
            let first_window = browser.driver.inner.window().await?;
            ensure!(starts.get() == 1 && quits.get() == 0);

            // 2. Recycled: same tab & document, but all per-test state cleared.
            let timings = Rc::new(RefCell::new(Timings::default()));
            let second = within(
                Some(&timings),
                super::run_one::<RecycleFixture>(
                    &mut browser,
                    Some(kept),
                    |driver, _| {
                        Box::pin(async move {
                            let is_top = driver
                                .inner
                                .execute("return window === window.top;", Vec::new())
                                .await?
                                .json()
                                .as_bool();
                            ensure!(is_top == Some(true), "driver stayed inside iframe");

                            let marker = driver
                                .inner
                                .execute("return window.__recycled_marker;", Vec::new())
                                .await?
                                .json()
                                .as_str()
                                .map(str::to_owned);
                            ensure!(
                                marker.as_deref() == Some("kept"),
                                "document was replaced: {marker:?}"
                            );

                            ensure!(
                                driver.waiting.clause().is_empty(),
                                "waiting was not cleared"
                            );
                            ensure!(
                                !driver.watching.load(Ordering::Relaxed),
                                "watching was not cleared"
                            );
                            ensure!(!driver.holding.take(), "holding was not cleared");
                            let leftovers = std::fs::read_dir(driver.downloads())?.count();
                            ensure!(leftovers == 0, "downloads were not cleared: {leftovers}");

                            let rect = driver.inner.get_window_rect().await?;
                            ensure!(
                                (rect.width as u32, rect.height as u32) == WINDOW,
                                "window size was not restored: {rect:?}"
                            );
                            Ok(())
                        })
                    },
                    setup(Mode::Keep),
                    None,
                ),
            )
            .await;
            second.result?;
            ensure!(second.warnings.is_empty(), "unexpected warnings");
            let kept = second
                .fixture
                .context("recycled pass should keep fixture")?;
            let second_window = browser.driver.inner.window().await?;
            ensure!(first_window == second_window, "recycle opened a new tab");
            ensure!(starts.get() == 1 && quits.get() == 0);

            let phases = timings
                .borrow()
                .phases
                .iter()
                .map(|(phase, _)| *phase)
                .collect::<Vec<_>>();
            ensure!(
                phases == [Phase::FixtureRecycle, Phase::TabReset, Phase::Body],
                "unexpected recycled phases: {phases:?}"
            );

            // 3. Declined recycle (Ok(false)): quits old, starts fresh on a new tab.
            let third = super::run_one::<RecycleFixture>(
                &mut browser,
                Some(kept),
                |driver, _| {
                    Box::pin(async move {
                        let marker = driver
                            .inner
                            .execute("return window.__recycled_marker ?? null;", Vec::new())
                            .await?;
                        ensure!(marker.json().is_null(), "fresh tab kept old marker");
                        Ok(())
                    })
                },
                setup(Mode::Decline),
                None,
            )
            .await;
            third.result?;
            ensure!(third.warnings.is_empty(), "unexpected warnings");
            let kept = third.fixture.context("passing test should keep fixture")?;
            let third_window = browser.driver.inner.window().await?;
            ensure!(third_window != second_window, "decline kept old tab");
            ensure!(starts.get() == 2 && quits.get() == 1);

            // 4. Errored recycle (Err): warns, quits old, starts fresh and passes.
            let fourth = super::run_one::<RecycleFixture>(
                &mut browser,
                Some(kept),
                |_, _| Box::pin(async { Ok(()) }),
                setup(Mode::Fail),
                None,
            )
            .await;
            fourth.result?;
            ensure!(fourth.warnings.len() == 1, "expected recycle warning");
            let warning = format!("{:#}", fourth.warnings[0]);
            ensure!(warning.contains("recycle-failed-marker"), "{warning}");
            let kept = fourth.fixture.context("passing test should keep fixture")?;
            ensure!(starts.get() == 3 && quits.get() == 2);

            // 5. Failing test: quits fixture immediately and does not keep it.
            let fifth = super::run_one::<RecycleFixture>(
                &mut browser,
                Some(kept),
                |_, _| Box::pin(async { anyhow::bail!("body-failed") }),
                setup(Mode::Keep),
                None,
            )
            .await;
            ensure!(fifth.result.is_err(), "failing test should fail");
            ensure!(
                fifth.fixture.is_none(),
                "failing test must not keep fixture"
            );
            ensure!(starts.get() == 3 && quits.get() == 3);

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
