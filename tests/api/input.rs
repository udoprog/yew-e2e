//! `send_keys`, `clear`, `focus`, `blur`, `focused_attr`, `set_value` and
//! `set_text`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged};

pub const PAGE: Page = Page {
    name: "input",
    body: r##"
<input data-test="name">
<output data-test="echo"></output>
<input data-test="color" type="color" value="#000000">
<p data-test="label">Original</p>
<div data-test="empty"></div>
<div data-test="quoted" tabindex="0" data-it's="yes">quoted</div>
"##,
    script: r##"
const name = document.querySelector("[data-test=name]");
const echo = document.querySelector("[data-test=echo]");
name.addEventListener("input", () => { echo.textContent = name.value; });
document.querySelector("[data-test=color]").addEventListener("change", e => record(`change ${e.target.value}`));
window.labelNode = document.querySelector("[data-test=label]").firstChild;
"##,
};

pub async fn send_keys_and_clear_edit_a_field(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let name = driver.find_one_by("[data-test=name]").await?;

    name.send_keys("hello").await?;
    driver.wait_texts("[data-test=echo]", ["hello"]).await?;
    ensure!(name.value().await? == "hello");

    name.clear().await?;
    ensure!(name.value().await?.is_empty(), "the field was not cleared");

    name.send_keys("again").await?;
    driver.wait_texts("[data-test=echo]", ["again"]).await
}

pub async fn focus_blur_and_focused_attr_follow_focus(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(
        driver.focused_attr("data-test").await?.is_none(),
        "something has focus on a fresh page"
    );

    driver
        .find_one_by("[data-test=name]")
        .await?
        .focus()
        .await?;
    ensure!(driver.focused_attr("data-test").await?.as_deref() == Some("name"));
    ensure!(driver.focused_attr("nothing").await?.is_none());

    driver.blur().await?;
    ensure!(
        driver.focused_attr("data-test").await?.is_none(),
        "focus stayed after blur"
    );

    // An attribute name with a quote in it is passed to the page as a value,
    // not spliced into the script.
    driver
        .find_one_by("[data-test=quoted]")
        .await?
        .focus()
        .await?;
    let quoted = driver.focused_attr("data-it's").await?;
    ensure!(quoted.as_deref() == Some("yes"), "read {quoted:?}");

    Ok(())
}

pub async fn set_value_fires_the_named_event(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let color = driver.find_one_by("[data-test=color]").await?;
    driver.set_value(&color, "#ff0000", "change").await?;

    logged(driver, "change #ff0000").await?;
    ensure!(color.value().await? == "#ff0000");
    Ok(())
}

pub async fn set_text_replaces_the_text_node(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let label = driver.find_one_by("[data-test=label]").await?;
    driver
        .set_text(&label, "A \"quoted\" name that is\nlonger")
        .await?;

    ensure!(label.prop("textContent").await? == "A \"quoted\" name that is\nlonger");

    let kept = crate::eval(
        driver,
        "return document.querySelector('[data-test=label]').firstChild === window.labelNode",
    )
    .await?;
    ensure!(
        kept == true,
        "the text node was replaced rather than changed"
    );
    Ok(())
}

pub async fn set_text_refuses_an_element_without_text(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let empty = driver.find_one_by("[data-test=empty]").await?;

    let Err(error) = driver.set_text(&empty, "anything").await else {
        bail!("an element without a text node was given text");
    };

    ensure!(
        format!("{error:#}").contains("no text node"),
        "the refusal said {error:#}"
    );
    Ok(())
}
