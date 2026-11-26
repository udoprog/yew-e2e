//! Scripts installed in the page: the clipboard, the WebSocket, the error
//! watch and the mutation count.

use std::sync::atomic::Ordering;

use anyhow::{Context, Result, bail, ensure};

use crate::{TestDriver, TestElement};

impl TestDriver {
    /// Capture clipboard writes in this page without accessing the system clipboard.
    ///
    /// Replaces `navigator.clipboard.writeText` so the text it is handed is
    /// kept on the page instead, where [`Self::wait_copied`] reads it. Lasts
    /// until the page is next loaded.
    pub async fn capture_clipboard(&self) -> Result<()> {
        let function = "() => { \
             navigator.clipboard.writeText = async text => { \
                 document.body.setAttribute('data-copied', text); \
             }; \
         }";

        self.inject(function, Vec::new()).await
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
        let function = "millis => { \
             WebSocket.__e2eOriginalSend = WebSocket.__e2eOriginalSend \
                 || WebSocket.prototype.send; \
             const send = WebSocket.__e2eOriginalSend; \
             WebSocket.prototype.send = function(data) { \
                 const copy = data && typeof data.slice === 'function' \
                     ? data.slice() : data; \
                 setTimeout(() => { if (this.readyState === 1) send.call(this, copy); }, \
                     millis); \
             }; \
         }";

        self.inject(function, vec![serde_json::json!(millis)]).await
    }

    /// Undo [`Self::delay_websocket_sends`], now that whatever it was
    /// installed to slow down has been observed. The patched `send` stays in
    /// effect for the rest of the page's life otherwise, taxing every later
    /// request on the same connection by the same delay -- fine for a test
    /// that stops looking right after, a trap for one that keeps going and
    /// pays it on every send from here to the next page load.
    pub async fn stop_delaying_websocket_sends(&self) -> Result<()> {
        let function = "() => { \
             if (WebSocket.__e2eOriginalSend) { \
                 WebSocket.prototype.send = WebSocket.__e2eOriginalSend; \
                 delete WebSocket.__e2eOriginalSend; \
             } \
         }";

        self.inject(function, Vec::new()).await
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
    pub(super) async fn rewatch(&self) -> Result<()> {
        if !self.watching.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.install_watch()
            .await
            .context("watching the page again after navigating")
    }

    async fn install_watch(&self) -> Result<()> {
        tracing::trace!("Listening for uncaught errors in the page");

        let function = "() => { \
             if (document.body.hasAttribute('data-watching')) { return; } \
             const say = what => document.body.setAttribute( \
                 'data-threw', String(what).slice(0, 6000)); \
             const told = (thrown, otherwise) => { \
                 if (!(thrown instanceof Error)) { return otherwise; } \
                 const head = String(thrown); \
                 const stack = thrown.stack || ''; \
                 return stack.startsWith(head) ? stack : head + '\\n' + stack; \
             }; \
             window.addEventListener('error', e => say(told(e.error, e.message))); \
             window.addEventListener('unhandledrejection', \
                 e => say(told(e.reason, e.reason))); \
             const was = console.error; \
             console.error = (...a) => { say(a.join(' ')); was(...a); }; \
             document.body.setAttribute('data-watching', ''); \
         }";

        self.inject(function, Vec::new()).await?;

        let watching = self
            .inner
            .execute(
                "return document.body.hasAttribute('data-watching');",
                Vec::new(),
            )
            .await?;

        ensure!(
            watching.json().as_bool() == Some(true),
            "the script that watches the page for errors did not run"
        );
        Ok(())
    }

    /// What the page threw since [`Self::watch_for_errors`], if anything did.
    pub async fn error_seen(&self) -> Result<Option<String>> {
        let body = self.find_one_by("body").await?;
        let seen = body.attr("data-threw").await?;

        Ok((!seen.is_empty()).then_some(seen))
    }

    /// **Fail if the page threw** since [`Self::watch_for_errors`], saying
    /// what it threw. The runner asks the same once a test has passed; this
    /// is for asking it at a point of the test's own choosing, such as before
    /// a step that would hide what went wrong.
    pub async fn ensure_no_errors(&self) -> Result<()> {
        match self.error_seen().await? {
            Some(threw) => bail!("the page threw: {threw}"),
            None => Ok(()),
        }
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
}
