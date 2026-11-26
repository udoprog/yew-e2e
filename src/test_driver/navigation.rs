//! Where the browser is: its window, its clock, waiting for a page or window
//! to show, and going to a page again, in a second tab or window, or in a
//! frame sized as a phone is.

use anyhow::{Context, Result, bail, ensure};
use thirtyfour::By;

use crate::driver::with_cleanup;
use crate::{TestDriver, WINDOW};

/// The attribute marking the frame [`TestDriver::in_frame`] makes.
pub(super) const FRAME_MARK: &str = "data-e2e-frame";

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

    /// Wait for the body to carry `data-page` = `page`, then watch the page
    /// for errors. Gives up after [`Self::load_timeout`] with
    /// [`Self::what_the_page_says`].
    pub async fn wait_page(&self, page: &str) -> Result<()> {
        tracing::trace!(page, "Waiting for page");

        let selector = By::Css(format!("body[data-page={}]", css_string(page)));

        let arrived = self
            .wait_until_within(
                self.load_timeout(),
                format_args!("page {page}"),
                async || {
                    Ok(!self
                        .find_all_within(self.load_timeout(), selector.clone())
                        .await?
                        .is_empty())
                },
            )
            .await;

        if let Err(error) = arrived {
            bail!(
                "{error}{}",
                self.what_the_page_says().await.unwrap_or_default()
            );
        }

        self.watch_for_errors().await
    }

    /// Go to `url` and [wait for `page`](Self::wait_page), returning the
    /// address the browser was at before.
    pub async fn visit(&self, url: &str, page: &str) -> Result<String> {
        let was = self.inner.current_url().await?;

        tracing::trace!(url, %was, page, "Visiting");

        self.inner.goto(url).await?;
        self.wait_page(page).await?;
        Ok(was.to_string())
    }

    /// Wait for a visible element whose `data-window` is `window`, then watch
    /// the page for errors. Gives up after [`Self::load_timeout`] with
    /// [`Self::what_the_page_says`].
    pub async fn wait_window(&self, window: &str) -> Result<()> {
        tracing::trace!(window, "Waiting for window");

        let selector = By::Css(format!("[data-window={}]", css_string(window)));

        let arrived = self
            .wait_until_within(
                self.load_timeout(),
                format_args!("window {window}"),
                async || {
                    for element in self
                        .find_all_within(self.load_timeout(), selector.clone())
                        .await?
                    {
                        if element.visible().await.unwrap_or(false) {
                            return Ok(true);
                        }
                    }

                    Ok(false)
                },
            )
            .await;

        if let Err(error) = arrived {
            bail!(
                "{error}{}",
                self.what_the_page_says().await.unwrap_or_default()
            );
        }

        self.watch_for_errors().await
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

    /// [`Self::in_second_tab`], [waiting for `window`](Self::wait_window)
    /// there before `run`.
    pub async fn in_new_tab(
        &self,
        window: &str,
        run: impl AsyncFnOnce() -> Result<()>,
    ) -> Result<()> {
        self.in_second_tab(async move || {
            self.wait_window(window).await?;
            run().await
        })
        .await
    }

    /// [`Self::in_second_window`], [waiting for `window`](Self::wait_window)
    /// there before `run`.
    pub async fn in_new_window(
        &self,
        window: &str,
        run: impl AsyncFnOnce() -> Result<()>,
    ) -> Result<()> {
        self.in_second_window(async move || {
            self.wait_window(window).await?;
            run().await
        })
        .await
    }

    /// **What a phone shows, in CSS pixels**: 390 wide and 844 tall, for
    /// [`Self::in_frame`].
    pub const PHONE: (u32, u32) = (390, 844);

    /// **Open the page this test is on again in a frame `width` by `height`
    /// pixels, and work inside it** until [`Self::leave_frame`].
    ///
    /// Firefox will not size a window below 500px wide, so a page cannot be
    /// shown as a phone shows it by sizing the window. A same-origin frame
    /// can be any size, and the page's media queries evaluate against the
    /// frame's. The window's document is emptied and made the frame's host,
    /// the window grown (never shrunk) until the frame fits in it whole, and
    /// the driver enters the frame once its document has loaded. A test
    /// watching for errors goes on watching the page in the frame.
    ///
    /// Already inside such a frame, the page the frame shows is the one
    /// opened again. [`Self::screenshot_png`] and [`Self::snapshot`] take the
    /// frame alone while the driver is in it.
    ///
    /// ```no_run
    /// # async fn phone(driver: &yew_e2e::TestDriver) -> anyhow::Result<()> {
    /// let (width, height) = yew_e2e::TestDriver::PHONE;
    /// driver.in_frame(width, height).await?;
    /// driver.snapshot("on-a-phone").await?;
    /// driver.leave_frame().await?;
    /// # Ok(()) }
    /// ```
    pub async fn in_frame(&self, width: u32, height: u32) -> Result<()> {
        self.in_frame_at("", width, height).await
    }

    /// [`Self::in_frame`] showing `path`, resolved against the page this test
    /// is on, in place of that page. It must stay on the page's origin, since
    /// a frame on another one could not be read from the window that hosts it.
    pub async fn in_frame_at(&self, path: &str, width: u32, height: u32) -> Result<()> {
        tracing::trace!(path, width, height, "Opening the page in a frame");

        let resolved = self
            .inner
            .execute(
                "const url = new URL(arguments[0], location.href); \
                 return url.origin === location.origin ? url.href : null;",
                vec![serde_json::json!(path)],
            )
            .await?;

        let Some(url) = resolved.json().as_str().map(str::to_owned) else {
            bail!("{path:?} is not on the origin of the page the frame would be opened from");
        };

        self.inner.enter_default_frame().await?;
        self.fit_window(width, height).await?;

        self.inner
            .execute(
                "const [url, width, height, mark] = arguments; \
                 document.open(); \
                 document.close(); \
                 const root = document.documentElement; \
                 root.style.overflow = 'hidden'; \
                 document.body.style.margin = '0'; \
                 const frame = document.createElement('iframe'); \
                 frame.setAttribute(mark, ''); \
                 frame.style.border = '0'; \
                 frame.style.display = 'block'; \
                 frame.style.width = width + 'px'; \
                 frame.style.height = height + 'px'; \
                 frame.src = url; \
                 document.body.appendChild(frame);",
                vec![
                    serde_json::json!(url),
                    serde_json::json!(width),
                    serde_json::json!(height),
                    serde_json::json!(FRAME_MARK),
                ],
            )
            .await?;

        let frame = self
            .find_one_by(By::Css(format!("iframe[{FRAME_MARK}]")))
            .await?;
        frame.inner.enter_frame().await?;

        let loaded = self
            .wait_until_within(
                self.load_timeout(),
                format_args!("the frame to load {url}"),
                async || {
                    // A script cut off by the frame's navigation answers
                    // nothing, and the frame starts out on `about:blank`.
                    let loaded = self
                        .inner
                        .execute(
                            "return location.href !== 'about:blank' \
                                 && document.readyState === 'complete';",
                            Vec::new(),
                        )
                        .await?;
                    Ok(loaded.json().as_bool() == Some(true))
                },
            )
            .await;

        if let Err(error) = loaded {
            bail!(
                "{error}{}",
                self.what_the_page_says().await.unwrap_or_default()
            );
        }

        let size = self
            .inner
            .execute(
                "return [window.innerWidth, window.innerHeight];",
                Vec::new(),
            )
            .await?;
        let size: (f64, f64) =
            serde_json::from_value(size.json().clone()).context("the frame's inner size")?;

        ensure!(
            size == (f64::from(width), f64::from(height)),
            "the frame shows the page {}x{} rather than {width}x{height}",
            size.0,
            size.1
        );

        self.rewatch().await
    }

    /// **Leave the frame [`Self::in_frame`] entered**, opening the page it
    /// shows in the window itself again. The window stays the size
    /// [`Self::in_frame`] grew it to. A test watching for errors goes on
    /// watching the page. Fails, leaving the driver where it is, if it is not
    /// in such a frame.
    pub async fn leave_frame(&self) -> Result<()> {
        tracing::trace!("Leaving the frame");

        ensure!(
            self.in_marked_frame().await?,
            "the driver is not in a frame in_frame entered"
        );

        let url = self
            .inner
            .execute("return location.href;", Vec::new())
            .await?;
        let url = url
            .json()
            .as_str()
            .context("the address of the page in the frame")?
            .to_owned();

        self.inner.enter_default_frame().await?;
        self.inner.goto(&url).await?;
        self.rewatch().await
    }

    /// Whether the driver is in the frame [`Self::in_frame`] made, and not
    /// in the window or a frame of the page's own.
    pub(super) async fn in_marked_frame(&self) -> Result<bool> {
        let framed = self
            .inner
            .execute(
                "const frame = window.frameElement; \
                 return !!frame && frame.hasAttribute(arguments[0]);",
                vec![serde_json::json!(FRAME_MARK)],
            )
            .await?;
        Ok(framed.json().as_bool() == Some(true))
    }

    /// Grow the window, never shrinking it, until what it shows of a page is
    /// at least `width` by `height`.
    async fn fit_window(&self, width: u32, height: u32) -> Result<()> {
        let sizes = self
            .inner
            .execute(
                "return [window.outerWidth, window.outerHeight, \
                     window.innerWidth, window.innerHeight];",
                Vec::new(),
            )
            .await?;
        let [outer_width, outer_height, inner_width, inner_height]: [u32; 4] =
            serde_json::from_value(sizes.json().clone()).context("the window's sizes")?;

        if inner_width >= width && inner_height >= height {
            return Ok(());
        }

        let grow = |outer: u32, inner: u32, wanted: u32| {
            outer.max(wanted.saturating_add(outer.saturating_sub(inner)))
        };

        self.set_window_size(
            grow(outer_width, inner_width, width),
            grow(outer_height, inner_height, height),
        )
        .await?;

        let shown = self
            .inner
            .execute(
                "return [window.innerWidth, window.innerHeight];",
                Vec::new(),
            )
            .await?;
        let [shown_width, shown_height]: [u32; 2] =
            serde_json::from_value(shown.json().clone()).context("the window's inner size")?;

        ensure!(
            shown_width >= width && shown_height >= height,
            "the window grew to show {shown_width}x{shown_height}, \
             short of a {width}x{height} frame"
        );
        Ok(())
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

fn css_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
