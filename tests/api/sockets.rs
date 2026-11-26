//! Holding the page's traffic: `track_websockets`, `hold_websocket_sends`, `held_websocket_sends`,
//! `release_websocket_sends`, `hold_fetches`, `held_fetches`,
//! `release_fetches`, `disconnect_websockets` and `reconnect_websockets`,
//! against an echo socket the fixture serves at `/socket`.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use yew_e2e::prelude::*;

use crate::{Page, Pages, log, logged, logged_exactly};

pub const PAGE: Page = Page {
    name: "sockets",
    body: r##"
<button data-test="connect">Connect</button>
<button data-test="send-a">Send a</button>
<button data-test="send-b">Send b</button>
<button data-test="send-bytes">Send bytes</button>
<button data-test="fetch-1">Fetch 1</button>
<button data-test="fetch-2">Fetch 2</button>
<button data-test="fetch-other">Fetch other</button>
"##,
    script: r##"
const on = (test, run) => document.querySelector(`[data-test=${test}]`).addEventListener("click", run);
let socket = null;
let opened = 0;
const connect = () => {
    const made = new WebSocket(`ws://${location.host}/socket`);
    const n = ++opened;
    made.binaryType = "arraybuffer";
    made.onopen = () => record(`open ${n}`);
    made.onclose = () => record(`closed ${n}`);
    made.onmessage = e => record(`echo ${typeof e.data === "string" ? e.data : new TextDecoder().decode(e.data)}`);
    socket = made;
};
connect();
on("connect", connect);
on("send-a", () => socket.send("a"));
on("send-b", () => socket.send("b"));
on("send-bytes", () => {
    // Sent from a buffer reused the moment send returns, as wasm does.
    const bytes = new TextEncoder().encode("bytes");
    socket.send(bytes);
    bytes.fill(33);
});
const get = (n, url) => fetch(url).then(r => r.text()).then(() => record(`fetched ${n}`));
on("fetch-1", () => get(1, "/download.json?n=1"));
on("fetch-2", () => get(2, "/download.json?n=2"));
on("fetch-other", () => get("other", "/sockets"));
"##,
};

/// Answer a WebSocket upgrade `request` on `stream`, then echo every frame
/// the client sends until it closes.
pub async fn echo(mut stream: TcpStream, request: &str) {
    let Some(key) = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("sec-websocket-key")
            .then(|| value.trim().to_owned())
    }) else {
        return;
    };

    let accept = base64(&sha1(
        format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
    ));

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\n\
         connection: Upgrade\r\nsec-websocket-accept: {accept}\r\n\r\n"
    );

    if stream.write_all(response.as_bytes()).await.is_err() {
        return;
    }

    while let Ok(Some((head, payload))) = frame(&mut stream).await {
        let opcode = head & 0x0f;

        let reply = match opcode {
            // A ping is answered with a pong.
            0x9 => 0x80 | 0xa,
            0xa => continue,
            _ => head,
        };

        let mut out = vec![reply];

        match payload.len() {
            n @ 0..126 => out.push(n as u8),
            n @ 126..=0xffff => {
                out.push(126);
                out.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                out.push(127);
                out.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }

        out.extend_from_slice(&payload);

        if stream.write_all(&out).await.is_err() || opcode == 0x8 {
            return;
        }
    }
}

/// One frame from the client: its first byte and its unmasked payload.
async fn frame(stream: &mut TcpStream) -> std::io::Result<Option<(u8, Vec<u8>)>> {
    let mut head = [0; 2];

    if stream.read_exact(&mut head).await.is_err() {
        return Ok(None);
    }

    let len = match head[1] & 0x7f {
        126 => u64::from(stream.read_u16().await?),
        127 => stream.read_u64().await?,
        n => u64::from(n),
    };

    let mut mask = [0; 4];

    if head[1] & 0x80 != 0 {
        stream.read_exact(&mut mask).await?;
    }

    let mut payload = vec![0; usize::try_from(len).unwrap_or(usize::MAX)];
    stream.read_exact(&mut payload).await?;

    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }

    Ok(Some((head[0], payload)))
}

/// SHA-1 of `data`, all a WebSocket handshake needs it for.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

    let mut message = data.to_vec();
    message.push(0x80);

    while message.len() % 64 != 56 {
        message.push(0);
    }

    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());

    for chunk in message.chunks(64) {
        let mut w = [0u32; 80];

        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }

        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = h;

        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..20 => ((b & c) | (!b & d), 0x5a827999),
                20..40 => (b ^ c ^ d, 0x6ed9eba1),
                40..60 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };

            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }

        for (h, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *h = h.wrapping_add(v);
        }
    }

    let mut out = [0; 20];

    for (out, h) in out.chunks_mut(4).zip(h) {
        out.copy_from_slice(&h.to_be_bytes());
    }

    out
}

/// Standard, padded base64 of `data`.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::new();

    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));

        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }

    out
}

/// Wait until `count` is what `held` reads.
async fn held_reach(
    driver: &TestDriver,
    what: &str,
    count: usize,
    held: impl AsyncFn() -> Result<usize>,
) -> Result<()> {
    driver
        .wait_until(format_args!("{count} {what} held"), async || {
            Ok(held().await? == count)
        })
        .await
}

pub async fn hold_websocket_sends_holds_and_releases_in_order(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;
    logged(driver, "open 1").await?;

    ensure!(
        driver.held_websocket_sends().await? == 0,
        "held before holding"
    );

    driver.hold_websocket_sends(None).await?;
    driver.press_nth("[data-test=send-a]", 0).await?;
    driver.press_nth("[data-test=send-bytes]", 0).await?;
    driver.press_nth("[data-test=send-b]", 0).await?;
    held_reach(driver, "sends", 3, async || {
        driver.held_websocket_sends().await
    })
    .await?;

    // Nothing reached the server while held.
    ensure!(log(driver).await? == ["open 1"], "a held send was answered");

    // Released in order, and the bytes are those sent, not the buffer reused
    // after.
    driver.release_websocket_sends().await?;
    logged_exactly(driver, ["open 1", "echo a", "echo bytes", "echo b"]).await?;
    ensure!(
        driver.held_websocket_sends().await? == 0,
        "still held after release"
    );

    // Holding what matches lets the rest through.
    driver.hold_websocket_sends(Some("b")).await?;
    driver.press_nth("[data-test=send-b]", 0).await?;
    driver.press_nth("[data-test=send-a]", 0).await?;
    logged(driver, "echo a").await?;
    held_reach(driver, "sends", 1, async || {
        driver.held_websocket_sends().await
    })
    .await?;

    driver.release_websocket_sends().await?;
    logged_exactly(
        driver,
        [
            "open 1",
            "echo a",
            "echo bytes",
            "echo b",
            "echo a",
            "echo b",
        ],
    )
    .await?;

    // Released, sends go straight through.
    driver.press_nth("[data-test=send-b]", 0).await?;
    logged_exactly(
        driver,
        [
            "open 1",
            "echo a",
            "echo bytes",
            "echo b",
            "echo a",
            "echo b",
            "echo b",
        ],
    )
    .await
}

pub async fn hold_fetches_holds_and_releases_in_order(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;
    logged(driver, "open 1").await?;

    ensure!(driver.held_fetches().await? == 0, "held before holding");

    driver.hold_fetches("download.json").await?;
    driver.press_nth("[data-test=fetch-2]", 0).await?;
    driver.press_nth("[data-test=fetch-1]", 0).await?;
    held_reach(driver, "fetches", 2, async || driver.held_fetches().await).await?;

    // One that does not match goes through meanwhile.
    driver.press_nth("[data-test=fetch-other]", 0).await?;
    logged_exactly(driver, ["open 1", "fetched other"]).await?;

    driver.release_fetches().await?;
    logged_exactly(
        driver,
        ["open 1", "fetched other", "fetched 2", "fetched 1"],
    )
    .await?;
    ensure!(
        driver.held_fetches().await? == 0,
        "still held after release"
    );

    driver.press_nth("[data-test=fetch-1]", 0).await?;
    logged_exactly(
        driver,
        [
            "open 1",
            "fetched other",
            "fetched 2",
            "fetched 1",
            "fetched 1",
        ],
    )
    .await
}

pub async fn disconnect_websockets_closes_and_refuses_until_reconnect(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;
    logged(driver, "open 1").await?;

    // The page's socket opened before tracking is known by sending on it, a
    // quiet one opened after by being opened.
    driver.track_websockets().await?;
    driver.track_websockets().await?;
    driver.press_nth("[data-test=send-a]", 0).await?;
    logged(driver, "echo a").await?;
    driver.press_nth("[data-test=connect]", 0).await?;
    logged(driver, "open 2").await?;

    driver.disconnect_websockets().await?;
    logged(driver, "closed 1").await?;
    logged(driver, "closed 2").await?;

    // A socket opened while disconnected never opens.
    driver.press_nth("[data-test=connect]", 0).await?;
    logged(driver, "closed 3").await?;

    driver.reconnect_websockets().await?;
    driver.press_nth("[data-test=connect]", 0).await?;
    logged(driver, "open 4").await?;
    driver.press_nth("[data-test=send-b]", 0).await?;
    logged(driver, "echo b").await?;

    ensure!(
        !log(driver).await?.iter().any(|line| line == "open 3"),
        "the refused socket opened: {:?}",
        log(driver).await?
    );
    Ok(())
}
