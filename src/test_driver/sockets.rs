//! Holding the page's network traffic: `WebSocket` sends held and released
//! in order, fetches held and released in order, and every socket closed and
//! kept closed until the test lets the page connect again.

use anyhow::Result;

use crate::TestDriver;

/// The state shared by the WebSocket calls, installed once per page as
/// `window.__e2eSockets` and bound to `s`.
///
/// What is held is a copy of what was sent: `send_with_u8_array` hands JS a
/// view over wasm linear memory, which the caller is free to reuse once
/// `send` returns (see [`TestDriver::delay_websocket_sends`]).
macro_rules! sockets {
    () => {
        "const s = window.__e2eSockets || (window.__e2eSockets = (() => { \
             const Real = window.WebSocket; \
             const send = Real.prototype.send; \
             const s = { sockets: new Set(), held: [], hold: null, refusing: false, send }; \
             window.WebSocket = new Proxy(Real, { \
                 construct(target, args, newTarget) { \
                     const socket = Reflect.construct(target, args, newTarget); \
                     s.sockets.add(socket); \
                     if (s.refusing) { socket.close(); } \
                     return socket; \
                 }, \
             }); \
             Real.prototype.send = function (data) { \
                 s.sockets.add(this); \
                 if (s.hold !== null) { \
                     const copy = ArrayBuffer.isView(data) \
                         ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice() \
                         : data instanceof ArrayBuffer ? data.slice(0) : data; \
                     const text = typeof copy === 'string' ? copy \
                         : copy instanceof Uint8Array || copy instanceof ArrayBuffer \
                             ? new TextDecoder().decode(copy) : null; \
                     if (s.hold === true || (text !== null && text.includes(s.hold))) { \
                         s.held.push([this, copy]); \
                         return; \
                     } \
                 } \
                 return send.call(this, data); \
             }; \
             return s; \
         })());"
    };
}

/// The state shared by the fetch calls, installed once per page as
/// `window.__e2eFetches` and bound to `f`.
macro_rules! fetches {
    () => {
        "const f = window.__e2eFetches || (window.__e2eFetches = (() => { \
             const real = window.fetch; \
             const f = { hold: null, held: [] }; \
             window.fetch = function (input, init) { \
                 if (f.hold !== null) { \
                     const given = input instanceof Request ? input.url : String(input); \
                     let url = given; \
                     try { url = new URL(given, location.href).href; } catch (e) {} \
                     if (url.includes(f.hold)) { \
                         return new Promise((resolve, reject) => f.held.push( \
                             () => real.call(window, input, init).then(resolve, reject))); \
                     } \
                 } \
                 return real.call(window, input, init); \
             }; \
             return f; \
         })());"
    };
}

impl TestDriver {
    /// **Start tracking this page's `WebSocket`s**, without holding or
    /// refusing anything, so a later [`Self::disconnect_websockets`] knows
    /// the sockets the page has meanwhile opened or sent on. Lasts until the
    /// page is next loaded.
    ///
    /// The first of the `WebSocket` calls in a page, this or any other,
    /// patches `WebSocket.prototype.send` and wraps the `WebSocket`
    /// constructor, so the sockets they know are those the page opens from
    /// then on and those it sends on from then on; one opened before and
    /// quiet since is not known. Tracking is idempotent. The patched `send`
    /// calls the `send` in place when it was installed, so
    /// [`Self::delay_websocket_sends`] composes with it only when it is
    /// installed after, and undone before the page is next loaded.
    pub async fn track_websockets(&self) -> Result<()> {
        self.inject(concat!("() => { ", sockets!(), " }"), Vec::new())
            .await
    }

    /// **Hold every `WebSocket` send in this page** whose payload contains
    /// `matching`, or every send at all with `None`, instead of sending it,
    /// until [`Self::release_websocket_sends`]: what the page shows while a
    /// request is unanswered stays there for as long as the test looks.
    ///
    /// A binary payload is matched as UTF-8 text, a `Blob` only by `None`.
    /// What is held is a copy taken when the page sent it, so a page reusing
    /// the buffer it sent from does not change what is finally sent. Holding
    /// again replaces what is matched and keeps what is already held. Lasts
    /// until released or the page is next loaded.
    ///
    /// Which sockets are seen is said at [`Self::track_websockets`].
    pub async fn hold_websocket_sends(&self, matching: Option<&str>) -> Result<()> {
        let function = concat!(
            "matching => { ",
            sockets!(),
            " s.hold = matching === null ? true : matching; }"
        );

        self.inject(function, vec![serde_json::json!(matching)])
            .await
    }

    /// How many `WebSocket` sends [`Self::hold_websocket_sends`] holds now.
    pub async fn held_websocket_sends(&self) -> Result<usize> {
        let held = self
            .inner
            .execute(
                "const s = window.__e2eSockets; return s ? s.held.length : 0;",
                Vec::new(),
            )
            .await?;
        Ok(held.convert::<usize>()?)
    }

    /// Stop holding `WebSocket` sends, and send what was held in the order
    /// the page sent it, each on the socket it was sent on. One whose socket
    /// is no longer open is dropped, as the network would have.
    pub async fn release_websocket_sends(&self) -> Result<()> {
        let function = concat!(
            "() => { ",
            sockets!(),
            " s.hold = null; \
             for (const [socket, data] of s.held.splice(0)) { \
                 if (socket.readyState === 1) { s.send.call(socket, data); } \
             } }"
        );

        self.inject(function, Vec::new()).await
    }

    /// **Hold every `fetch` in this page** whose URL contains `url_part`, or
    /// every fetch with an empty one, instead of making it, until
    /// [`Self::release_fetches`]. The URL matched is the absolute one, the
    /// page's own resolved against the page's address. Holding again replaces what is matched and keeps
    /// what is already held. Lasts until released or the page is next
    /// loaded.
    pub async fn hold_fetches(&self, url_part: &str) -> Result<()> {
        let function = concat!("part => { ", fetches!(), " f.hold = part; }");

        self.inject(function, vec![serde_json::json!(url_part)])
            .await
    }

    /// How many fetches [`Self::hold_fetches`] holds now.
    pub async fn held_fetches(&self) -> Result<usize> {
        let held = self
            .inner
            .execute(
                "const f = window.__e2eFetches; return f ? f.held.length : 0;",
                Vec::new(),
            )
            .await?;
        Ok(held.convert::<usize>()?)
    }

    /// Stop holding fetches, and make those held one after the other in the
    /// order the page made them, each answered before the next is made.
    pub async fn release_fetches(&self) -> Result<()> {
        let function = concat!(
            "() => { ",
            fetches!(),
            " f.hold = null; \
             const held = f.held.splice(0); \
             (async () => { for (const go of held) { await go(); } })(); }"
        );

        self.inject(function, Vec::new()).await
    }

    /// **Close every `WebSocket` the page holds and refuse new ones** until
    /// [`Self::reconnect_websockets`], the way a server going away would:
    /// a socket the page opens meanwhile fails before it connects, with the
    /// `error` and `close` events a refused connection has. Waits for each
    /// socket closed to finish closing. Which sockets are known is said at
    /// [`Self::track_websockets`].
    pub async fn disconnect_websockets(&self) -> Result<()> {
        let function = concat!(
            "() => { ",
            sockets!(),
            " s.refusing = true; \
             for (const socket of s.sockets) { socket.close(); } }"
        );

        self.inject(function, Vec::new()).await?;

        self.wait_until("every WebSocket the page holds to close", async || {
            let closed = self
                .inner
                .execute(
                    "const s = window.__e2eSockets; \
                     return !s || [...s.sockets].every(socket => socket.readyState === 3);",
                    Vec::new(),
                )
                .await?;
            Ok(closed.json().as_bool() == Some(true))
        })
        .await
    }

    /// Let the page open `WebSocket`s again after
    /// [`Self::disconnect_websockets`]. Whether and when it does is the
    /// page's own: this only stops refusing.
    pub async fn reconnect_websockets(&self) -> Result<()> {
        let function = concat!(
            "() => { ",
            sockets!(),
            " s.refusing = false; \
             for (const socket of [...s.sockets]) { \
                 if (socket.readyState === 3) { s.sockets.delete(socket); } \
             } }"
        );

        self.inject(function, Vec::new()).await
    }
}
