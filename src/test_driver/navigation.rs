//! Where the browser is: its window, its clock, and going to a page again or
//! in a second tab or window.

use anyhow::{Context, Result, bail};

use crate::driver::with_cleanup;
use crate::{TestDriver, WINDOW};

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

    /// Resize the browser window.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<()> {
        tracing::trace!(width, height, "Sizing the window");
        self.inner.set_window_rect(0, 0, width, height).await?;
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
}
