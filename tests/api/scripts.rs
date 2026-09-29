//! What is injected into the page: `watch_for_errors`, `error_seen`,
//! `capture_clipboard`, `wait_copied`, `delay_websocket_sends`,
//! `stop_delaying_websocket_sends`, `watch_mutations` and `mutations`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, eval, logged_exactly};

pub const PAGE: Page = Page {
    name: "scripts",
    body: r##"
<button data-test="throw">Throw</button>
<button data-test="reject">Reject</button>
<button data-test="complain">Complain</button>
<button data-test="copy">Copy</button>
<button data-test="send">Send</button>
<button data-test="mutate">Mutate</button>
<button data-test="redraw">Redraw</button>
<button data-test="remove">Remove</button>
<button data-test="declare">Declare</button>
<div data-test="watched"><span class="leaf">leaf</span><canvas width="10" height="10"></canvas></div>
"##,
    script: r##"
const on = (test, run) => document.querySelector(`[data-test=${test}]`).addEventListener("click", run);
on("throw", () => setTimeout(() => { throw new Error("boom"); }));
on("reject", () => { Promise.reject("nope"); });
on("complain", () => console.error("complained", 1));
on("copy", () => { navigator.clipboard.writeText("copied text"); });
const original = WebSocket.prototype.send;
on("send", () => record(WebSocket.prototype.send === original ? "original send" : "patched send"));
const watched = document.querySelector("[data-test=watched]");
on("mutate", () => {
    watched.dataset.state = "on";
    watched.append(document.createElement("i"));
});
on("redraw", () => watched.querySelector("canvas").getContext("2d").clearRect(0, 0, 10, 10));
on("remove", () => watched.remove());
on("declare", () => {
    const script = document.createElement("script");
    script.textContent = "const say = 'said'; const was = 'was'; record(`declared ${say} ${was}`);";
    document.body.append(script);
});
"##,
};

/// A page whose own globals have the names the error watcher uses inside.
pub const COLLIDE: Page = Page {
    name: "collide",
    body: r##"
<button data-test="reject">Reject</button>
"##,
    script: r##"
const say = "the page's say";
let was = "the page's was";
document.querySelector("[data-test=reject]").addEventListener("click", () => {
    Promise.reject(`collided with ${say} and ${was}`);
});
"##,
};

/// Forget what the page threw, once the test has heard it, so the runner
/// does not fail the test for it.
async fn forget_the_throw(driver: &TestDriver) -> Result<()> {
    eval(driver, "document.body.removeAttribute('data-threw')").await?;
    ensure!(
        driver.error_seen().await?.is_none(),
        "the throw was not forgotten"
    );
    Ok(())
}

/// Wait for the page to have thrown something that says `what`.
async fn threw(driver: &TestDriver, what: &str) -> Result<()> {
    driver
        .wait_until(format_args!("the page to throw {what:?}"), async || {
            Ok(driver
                .error_seen()
                .await?
                .is_some_and(|threw| threw.contains(what)))
        })
        .await
}

pub async fn watch_for_errors_hears_a_throw(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    // Watching twice installs one listener, not two.
    driver.watch_for_errors().await?;
    ensure!(driver.error_seen().await?.is_none(), "a quiet page threw");

    driver.press_nth("[data-test=throw]", 0).await?;
    // What it said, and where it was thrown.
    threw(driver, "Error: boom").await?;
    ensure!(
        driver
            .error_seen()
            .await?
            .is_some_and(|threw| threw.contains(&format!("{}:", pages.address("scripts")))),
        "the throw did not say where it was thrown"
    );
    forget_the_throw(driver).await?;

    driver.press_nth("[data-test=complain]", 0).await?;
    threw(driver, "complained 1").await?;
    forget_the_throw(driver).await
}

pub async fn watch_for_errors_hears_a_rejection(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.press_nth("[data-test=reject]", 0).await?;
    threw(driver, "nope").await?;
    forget_the_throw(driver).await
}

pub async fn watch_for_errors_keeps_out_of_the_page_globals(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    // A page with globals of the watcher's names is still watched.
    pages.open(driver, &COLLIDE).await?;
    driver.press_nth("[data-test=reject]", 0).await?;
    threw(driver, "collided with the page's say and the page's was").await?;
    forget_the_throw(driver).await?;

    // And a page that declares them once it is watched still can.
    pages.open(driver, &PAGE).await?;
    driver.press_nth("[data-test=declare]", 0).await?;
    logged_exactly(driver, ["declared said was"]).await
}

pub async fn capture_clipboard_and_wait_copied(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.capture_clipboard().await?;
    driver.press_nth("[data-test=copy]", 0).await?;
    driver.wait_copied("copied text").await
}

pub async fn delay_websocket_sends_patches_and_restores_send(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.press_nth("[data-test=send]", 0).await?;
    driver.delay_websocket_sends(10).await?;
    // A second delay does not stack on the first: the one it restores is
    // still the page's own.
    driver.delay_websocket_sends(20).await?;
    driver.press_nth("[data-test=send]", 0).await?;
    driver.stop_delaying_websocket_sends().await?;
    driver.press_nth("[data-test=send]", 0).await?;

    logged_exactly(driver, ["original send", "patched send", "original send"]).await
}

pub async fn watch_mutations_counts_changes(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(
        driver.mutations().await? == (0, Vec::new()),
        "counted before watching"
    );

    let watched = driver.find_one_by("[data-test=watched]").await?;
    driver.watch_mutations(&watched).await?;

    driver.press_nth("[data-test=mutate]", 0).await?;
    let seen = mutations_reach(driver, 2).await?;
    ensure!(
        seen == ["attributes:data-state:DIV", "childList::DIV"],
        "saw {seen:?}"
    );

    // Watching again starts over.
    driver.watch_mutations(&watched).await?;
    ensure!(
        driver.mutations().await? == (0, Vec::new()),
        "the count carried over"
    );

    driver.press_nth("[data-test=redraw]", 0).await?;
    driver.press_nth("[data-test=remove]", 0).await?;
    let seen = mutations_reach(driver, 2).await?;
    ensure!(seen == ["redrawn", "removed"], "saw {seen:?}");
    Ok(())
}

/// Wait for [`TestDriver::mutations`] to have counted `count`, and say what
/// they were.
async fn mutations_reach(driver: &TestDriver, count: u64) -> Result<Vec<String>> {
    driver
        .wait_until(format_args!("{count} mutations"), async || {
            Ok(driver.mutations().await?.0 >= count)
        })
        .await?;

    let (seen, kinds) = driver.mutations().await?;
    ensure!(seen == count, "counted {seen}: {kinds:?}");
    Ok(kinds)
}
