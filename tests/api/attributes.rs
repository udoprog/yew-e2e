//! `text`, `attr`, `prop`, `value`, `css`, `rect`, `visible`, `pressed`,
//! `enabled`, `disabled`, `element`, `find_all_attrs` and `bands`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, near};

pub const PAGE: Page = Page {
    name: "attributes",
    body: r##"
<input data-test="field" value="initial" title="a field">
<button data-test="toggle" aria-pressed="true">Toggle</button>
<button data-test="off" disabled>Off</button>
<div data-test="custom" role="button" aria-disabled="true" tabindex="0">Custom</div>
<div data-test="styled" style="position: absolute; left: 400px; top: 200px; width: 120px; height: 40px; color: rgb(255, 0, 0)">Styled</div>
<div data-test="hidden" style="display: none">hidden</div>
<ul data-test="rows" style="margin: 0; padding: 0; list-style: none">
<li data-id="a" style="height: 20px">A</li><li data-id="b" style="height: 30px">B</li><li data-id="c" style="display: none">C</li><li>none</li>
</ul>
<p data-test="text">plain <b>bold</b></p>
"##,
    script: "",
};

pub async fn text_attr_and_prop_read_the_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(driver.find_one_by("[data-test=text]").await?.text().await? == "plain bold");

    let field = driver.find_one_by("[data-test=field]").await?;
    ensure!(field.attr("title").await? == "a field");
    ensure!(field.attr("nothing").await?.is_empty());
    ensure!(field.prop("value").await? == "initial");
    ensure!(field.prop("nothing").await?.is_empty());
    ensure!(field.value().await? == "initial");

    field.send_keys("!").await?;
    ensure!(field.value().await? == "initial!", "the value property");
    ensure!(
        field.attr("value").await? == "initial",
        "the value attribute moved with the property"
    );

    Ok(())
}

pub async fn element_hands_back_the_webdriver_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let field = driver.find_one_by("[data-test=field]").await?;
    ensure!(field.element().tag_name().await? == "input");
    Ok(())
}

pub async fn css_and_rect_measure_the_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let styled = driver.find_one_by("[data-test=styled]").await?;
    ensure!(styled.css("color").await? == "rgb(255, 0, 0)");
    ensure!(styled.css("width").await? == "120px");

    let rect = styled.rect().await?;
    near("left", rect.x, 400.0, 0.5)?;
    near("top", rect.y, 200.0, 0.5)?;
    near("width", rect.width, 120.0, 0.5)?;
    near("height", rect.height, 40.0, 0.5)?;
    Ok(())
}

pub async fn visible_follows_display(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(
        driver
            .find_one_by("[data-test=styled]")
            .await?
            .visible()
            .await?
    );
    ensure!(
        !driver
            .find_one_by("[data-test=hidden]")
            .await?
            .visible()
            .await?
    );
    Ok(())
}

pub async fn pressed_reads_aria_pressed(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(
        driver
            .find_one_by("[data-test=toggle]")
            .await?
            .pressed()
            .await?
    );
    ensure!(
        !driver
            .find_one_by("[data-test=off]")
            .await?
            .pressed()
            .await?
    );
    Ok(())
}

pub async fn enabled_and_disabled_read_their_own_attributes(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let toggle = driver.find_one_by("[data-test=toggle]").await?;
    let off = driver.find_one_by("[data-test=off]").await?;
    let custom = driver.find_one_by("[data-test=custom]").await?;

    ensure!(toggle.enabled().await? && !toggle.disabled().await?);
    ensure!(!off.enabled().await?, "a disabled button is enabled");
    ensure!(
        custom.disabled().await?,
        "aria-disabled did not read as disabled"
    );
    ensure!(
        custom.enabled().await?,
        "a custom control cannot carry `disabled`, so it reads as enabled"
    );
    Ok(())
}

pub async fn find_all_attrs_reads_every_match(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let ids = driver
        .find_all_attrs("[data-test=rows] li", "data-id")
        .await?;
    ensure!(ids == ["a", "b", "c", ""], "read {ids:?}");
    ensure!(driver.find_all_attrs(".none", "data-id").await?.is_empty());
    Ok(())
}

pub async fn bands_measure_what_is_drawn(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let bands = driver.bands("[data-test=rows] li", "data-id").await?;
    let names = bands
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect::<Vec<_>>();

    ensure!(
        names == ["a", "b", ""],
        "an undrawn row was measured, or a drawn one was not: {bands:?}"
    );

    let (_, a_top, a_height) = bands[0];
    let (_, b_top, b_height) = bands[1];
    near("a's height", a_height, 20.0, 0.5)?;
    near("b's height", b_height, 30.0, 0.5)?;
    near("b's top", b_top, a_top + 20.0, 0.5)?;
    Ok(())
}
