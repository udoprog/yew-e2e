//! `scroll_top`, `scroll_to_far_end` and `scroll_into_view`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, eval, logged};

pub const PAGE: Page = Page {
    name: "scroll",
    body: r##"
<div data-test="scroller" style="height: 100px; overflow: auto"><div style="height: 1000px">tall</div></div>
<div data-test="wide" style="width: 200px; overflow-x: auto"><div style="width: 2000px"><span data-test="inside">inside</span></div></div>
<p data-test="far" style="margin-top: 3000px">far</p>
"##,
    script: r##"
const scroller = document.querySelector("[data-test=scroller]");
scroller.addEventListener("scroll", () => record(`scrolled ${scroller.scrollTop}`));
"##,
};

pub async fn scroll_top_scrolls_a_container(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let scroller = driver.find_one_by("[data-test=scroller]").await?;
    driver.scroll_top(&scroller, 250).await?;

    logged(driver, "scrolled 250").await?;
    ensure!(scroller.prop("scrollTop").await? == "250");
    Ok(())
}

pub async fn scroll_to_far_end_scrolls_the_nearest_scroller(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let inside = driver.find_one_by("[data-test=inside]").await?;
    driver.scroll_to_far_end(&inside).await?;

    let travel = eval(
        driver,
        "const wide = document.querySelector('[data-test=wide]'); \
         return [wide.scrollLeft, wide.scrollWidth - wide.clientWidth];",
    )
    .await?;

    let left = travel[0].as_f64().unwrap_or_default();
    let end = travel[1].as_f64().unwrap_or_default();

    ensure!(end > 0.0, "the box has nowhere to scroll: {travel}");
    crate::near("scrollLeft", left, end, 1.0)
}

pub async fn scroll_into_view_brings_an_element_into_view(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let in_view = "const box = document.querySelector('[data-test=far]').getBoundingClientRect(); \
                   return box.top >= 0 && box.bottom <= window.innerHeight;";

    ensure!(
        eval(driver, in_view).await? == false,
        "the far paragraph starts in view"
    );

    driver
        .find_one_by("[data-test=far]")
        .await?
        .scroll_into_view()
        .await?;

    ensure!(
        eval(driver, in_view).await? == true,
        "the far paragraph is still out of view"
    );
    Ok(())
}
