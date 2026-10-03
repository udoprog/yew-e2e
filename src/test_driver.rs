//! [`TestDriver`], the browser a test is handed, and its methods group by
//! group.

use std::sync::atomic::AtomicBool;

use anyhow::{Result, ensure};
use thirtyfour::prelude::*;
use tokio::time::{self, Duration};

use crate::finding::common;
use crate::{Holding, IntoBy, TestElement, Waiting};

mod attributes;
mod canvas;
mod capture;
mod drag;
mod input;
mod keyboard;
mod navigation;
mod pointer;
mod scripts;
mod scroll;
mod storage;
mod style;
mod wheel;

pub use self::canvas::CanvasSize;

/// A number handed to a page script as an argument, refused unless it is
/// finite: JSON has no infinity or NaN, so the page would read `null` instead.
fn finite(what: &str, value: f64) -> Result<serde_json::Value> {
    ensure!(value.is_finite(), "{what} must be finite, not {value}");
    Ok(serde_json::json!(value))
}

/// Wrapper for `WebDriver` which adds convenience methods for testing.
pub struct TestDriver {
    pub(crate) inner: WebDriver,
    pub(crate) downloads: tempfile::TempDir,
    pub(crate) wait_timeout: Duration,
    pub(crate) load_timeout: Duration,
    pub(crate) waiting: Waiting,
    /// Whether this test asked for [`TestDriver::watch_for_errors`], so the
    /// helpers that navigate know to watch the page they arrive at too.
    pub(crate) watching: AtomicBool,
    pub(crate) holding: Holding,
}

impl TestDriver {
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

    /// **Run `function`, a JavaScript function's source, as the page's own
    /// script**, called with `args`.
    ///
    /// A WebDriver script runs apart from the page in some browsers (Firefox
    /// runs it in a sandbox), so what it puts on `window` or a prototype is
    /// not what the page sees. A `<script>` element added to the document
    /// runs as the page, and runs at once; it cannot read `arguments`, so the
    /// call it makes is built in the page, with `args` as JSON.
    async fn inject(&self, function: &str, args: Vec<serde_json::Value>) -> Result<()> {
        let script = "const [source, args] = arguments; \
             const inject = document.createElement('script'); \
             inject.textContent = `(${source})(...${JSON.stringify(args)});`; \
             document.documentElement.appendChild(inject); \
             inject.remove();";

        self.inner
            .execute(
                script,
                vec![serde_json::json!(function), serde_json::json!(args)],
            )
            .await?;
        Ok(())
    }

    common!();
}
