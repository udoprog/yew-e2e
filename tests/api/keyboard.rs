//! `press_key`, `hold_key`, `release_key`, `press_key_on`,
//! `press_key_with_ctrl` and `press_key_with_shift`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged_exactly};

pub const PAGE: Page = Page {
    name: "keyboard",
    body: r##"
<div data-test="target" tabindex="0">target</div>
"##,
    script: r##"
for (const type of ["keydown", "keyup"]) {
    document.addEventListener(type, e => {
        const mods = (e.ctrlKey ? " ctrl" : "") + (e.shiftKey ? " shift" : "") + (e.altKey ? " alt" : "");
        const at = e.target.dataset.test || e.target.localName;
        record(`${e.type} ${e.key}${mods} @${at}`);
        if (e.type === "keydown") document.body.dataset.code = e.code;
    });
}
"##,
};

pub async fn press_key_lands_on_the_page(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.press_key(Key::Escape).await?;
    logged_exactly(driver, ["keydown Escape @body", "keyup Escape @body"]).await
}

pub async fn hold_key_and_release_key_hold_across_observations(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.hold_key('a').await?;
    logged_exactly(driver, ["keydown a @body"]).await?;

    driver.release_key('a').await?;
    logged_exactly(driver, ["keydown a @body", "keyup a @body"]).await
}

pub async fn press_key_on_dispatches_any_named_key(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .press_key_on("[data-test=target]", "MediaPlayPause")
        .await?;
    logged_exactly(driver, ["keydown MediaPlayPause @target"]).await?;
    ensure!(driver.focused_attr("data-test").await?.as_deref() == Some("target"));

    let Err(error) = driver.press_key_on("[data-test=nothing]", "a").await else {
        bail!("a key was pressed on nothing");
    };
    ensure!(
        format!("{error:#}").contains("No element for keyboard press"),
        "{error:#}"
    );
    Ok(())
}

pub async fn press_key_with_ctrl_carries_the_code(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .press_key_with_ctrl("[data-test=target]", "z")
        .await?;
    logged_exactly(driver, ["keydown z ctrl @target"]).await?;

    let body = driver.find_one_by("body").await?;
    ensure!(body.attr("data-code").await? == "KeyZ");

    driver
        .press_key_with_ctrl("[data-test=target]", "Enter")
        .await?;
    logged_exactly(
        driver,
        ["keydown z ctrl @target", "keydown Enter ctrl @target"],
    )
    .await?;
    ensure!(body.attr("data-code").await? == "Enter");
    Ok(())
}

pub async fn press_key_with_shift_holds_shift(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .press_key_with_shift("[data-test=target]", "Tab")
        .await?;
    logged_exactly(driver, ["keydown Tab shift @target"]).await
}
