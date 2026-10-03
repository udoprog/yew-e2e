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
        let webdriver = Driver::new(engine, deadline).await?;
        Self::connect(webdriver, caps, deadline).await
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

    /// Put the browser on a fresh tab, then go to `url`.
    ///
    /// The preceding document is closed rather than navigated away from. A
    /// WebDriver navigation leaves session history behind; long runs showed
    /// memory growth that closing the previous tab removed.
    async fn reset(&mut self, url: &str) -> Result<()> {
        self.driver.waiting.clear();
        self.driver.watching.store(false, Ordering::Relaxed);
        // A drag the last test left held is not this test's to let go of where
        // the pointer is: its release_all_input must not send a pointerup into
        // a page that never saw the press.
        self.driver.holding.set(false);
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

        write_chain(&mut errors, "Test error", error)?;
    }

    if let Err(error) = fixture_result {
        write_chain(&mut errors, "Fixture error", &error)?;
    }

    if !errors.is_empty() {
        return Err(anyhow!("{errors}"));
    }

    Ok(())
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
