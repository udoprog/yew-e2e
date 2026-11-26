//! `in_frame`, `in_frame_at`, `leave_frame` and `TestDriver::PHONE`, and the error watch across
//! them.

use yew_e2e::prelude::*;

use crate::{Page, Pages, eval};

pub const PAGE: Page = Page {
    name: "frame",
    body: r##"
<style>
[data-test=layout]::after { content: "wide"; }
@media (max-width: 499px) { [data-test=layout]::after { content: "narrow"; } }
</style>
<output data-test="layout"></output>
<output data-test="query"></output>
<button data-test="throw">Throw</button>
"##,
    script: r##"
document.querySelector("[data-test=query]").textContent = location.search;
document.querySelector("[data-test=throw]").addEventListener("click", () => {
    setTimeout(() => { throw new Error("thrown in the frame"); });
});
"##,
};

/// What the `layout` output's media query says, in the document the driver is in.
async fn layout(driver: &TestDriver) -> Result<serde_json::Value> {
    eval(
        driver,
        "return getComputedStyle(document.querySelector('[data-test=layout]'), '::after').content",
    )
    .await
}

/// Whether the driver is in a frame of the window's own document, and not in
/// the window or a frame nested deeper.
async fn framed(driver: &TestDriver) -> Result<bool> {
    Ok(eval(
        driver,
        "return window.self !== window.top && window.parent === window.top",
    )
    .await?
        == true)
}

pub async fn in_frame_shows_the_page_at_the_frames_size(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;
    ensure!(layout(driver).await? == "\"wide\"", "the window is narrow");

    let (width, height) = TestDriver::PHONE;
    ensure!((width, height) == (390, 844), "a phone is {width}x{height}");
    driver.in_frame(width, height).await?;

    ensure!(framed(driver).await?, "the driver is not in the frame");
    let size = eval(driver, "return [window.innerWidth, window.innerHeight]").await?;
    ensure!(size == serde_json::json!([390, 844]), "the frame is {size}");
    ensure!(
        layout(driver).await? == "\"narrow\"",
        "the media query did not see a phone"
    );
    ensure!(
        eval(driver, "return location.pathname").await? == "/frame",
        "the frame shows another page"
    );
    driver.find_one_by("body[data-page=frame]").await?;

    // The window grew to hold the frame whole.
    driver.webdriver().enter_default_frame().await?;
    let inner = eval(driver, "return window.innerHeight").await?;
    ensure!(
        inner.as_u64().is_some_and(|inner| inner >= 844),
        "the window shows {inner}px of the frame's 844"
    );
    Ok(())
}

pub async fn in_frame_keeps_the_watch(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.in_frame(390, 600).await?;
    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
        "the page in the frame is not watched"
    );

    driver.press_nth("[data-test=throw]", 0).await?;
    driver
        .wait_until("the frame's throw to be heard", async || {
            Ok(driver.error_seen().await?.is_some())
        })
        .await?;

    let threw = driver.error_seen().await?.unwrap_or_default();
    ensure!(threw.contains("thrown in the frame"), "heard {threw:?}");

    // Heard, and forgotten, so the test itself passes.
    eval(driver, "document.body.removeAttribute('data-threw')").await?;
    Ok(())
}

pub async fn in_frame_leaves_an_unwatched_test_unwatched(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    driver.webdriver().goto(pages.address(PAGE.name)).await?;
    driver.find_one_by("body[data-page=frame]").await?;

    driver.in_frame(390, 600).await?;
    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == false,
        "the frame is watched for a test that never asked"
    );
    Ok(())
}

pub async fn in_frame_at_opens_a_path_on_the_page_origin(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.in_frame_at("frame?at=1", 390, 600).await?;
    driver.wait_texts("[data-test=query]", ["?at=1"]).await?;

    // Inside the frame, a relative path resolves against the frame's page,
    // and the frame is opened again rather than nested.
    driver.in_frame_at("/frame?again=1", 400, 500).await?;
    driver.wait_texts("[data-test=query]", ["?again=1"]).await?;
    ensure!(framed(driver).await?, "the frame is nested or gone");
    let size = eval(driver, "return [window.innerWidth, window.innerHeight]").await?;
    ensure!(size == serde_json::json!([400, 500]), "the frame is {size}");
    Ok(())
}

pub async fn in_frame_again_reopens_the_page_in_the_frame(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.in_frame_at("frame?kept=1", 390, 600).await?;
    driver.in_frame(390, 844).await?;
    driver.wait_texts("[data-test=query]", ["?kept=1"]).await?;
    ensure!(framed(driver).await?, "the frame is nested or gone");
    Ok(())
}

pub async fn in_frame_at_refuses_another_origin(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver
        .in_frame_at("http://elsewhere.invalid/frame", 390, 844)
        .await
    else {
        bail!("opened a frame on another origin");
    };

    let said = format!("{error:#}");
    ensure!(said.contains("not on the origin"), "it said {said:?}");
    ensure!(
        !framed(driver).await?,
        "the driver went into a frame anyway"
    );
    driver.find_one_by("body[data-page=frame]").await?;
    Ok(())
}

pub async fn leave_frame_opens_the_page_in_the_window(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.in_frame_at("frame?left=1", 390, 844).await?;
    driver.leave_frame().await?;

    ensure!(
        eval(driver, "return window.self === window.top").await? == true,
        "the driver is still in a frame"
    );
    driver.wait_texts("[data-test=query]", ["?left=1"]).await?;
    ensure!(layout(driver).await? == "\"wide\"", "the window is narrow");
    ensure!(
        driver.count("iframe").await? == 0,
        "the window still hosts the frame"
    );
    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
        "the page is not watched after leaving the frame"
    );
    Ok(())
}

pub async fn leave_frame_refuses_outside_a_frame(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver.leave_frame().await else {
        bail!("left a frame the driver was never in");
    };

    let said = format!("{error:#}");
    ensure!(said.contains("not in a frame"), "it said {said:?}");
    Ok(())
}
