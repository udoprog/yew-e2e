//! `mark_page`, `page_kept`, `mark` and `kept_marked`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged};

pub const PAGE: Page = Page {
    name: "marks",
    body: r##"
<ul data-test="items"><li>one</li><li>two</li><li>three</li></ul>
<button data-test="retext">Retext</button>
<button data-test="replace">Replace</button>
<button data-test="route">Route</button>
"##,
    script: r##"
const items = document.querySelector("[data-test=items]");
document.querySelector("[data-test=retext]").addEventListener("click", () => {
    for (const item of items.children) item.textContent += "!";
    record("retexted");
});
document.querySelector("[data-test=replace]").addEventListener("click", () => {
    const last = items.lastElementChild;
    const copy = document.createElement("li");
    copy.textContent = last.textContent;
    last.replaceWith(copy);
    record("replaced");
});
document.querySelector("[data-test=route]").addEventListener("click", () => {
    history.pushState(null, "", "/marks?routed=1");
    record("routed");
});
"##,
};

pub async fn page_kept_tells_client_side_navigation_from_reload(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(!driver.page_kept().await?, "a page never marked was kept");

    driver.mark_page().await?;
    ensure!(driver.page_kept().await?, "a page just marked was not kept");

    driver.press_nth("[data-test=route]", 0).await?;
    logged(driver, "routed").await?;
    ensure!(
        driver.page_kept().await?,
        "a client-side navigation lost the mark"
    );

    driver.reload().await?;
    ensure!(!driver.page_kept().await?, "a reload kept the mark");
    Ok(())
}

pub async fn kept_marked_counts_elements_a_render_kept(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(driver.kept_marked("[data-test=items] li").await? == 0);

    let marked = driver.mark("[data-test=items] li").await?;
    ensure!(marked == 3, "marked {marked}");
    ensure!(driver.kept_marked("[data-test=items] li").await? == 3);

    driver.press_nth("[data-test=retext]", 0).await?;
    logged(driver, "retexted").await?;
    let kept = driver.kept_marked("[data-test=items] li").await?;
    ensure!(kept == 3, "changing their text kept {kept}");

    driver.press_nth("[data-test=replace]", 0).await?;
    logged(driver, "replaced").await?;
    let kept = driver.kept_marked("[data-test=items] li").await?;
    ensure!(kept == 2, "replacing one kept {kept}");

    // The mark is not an attribute the page could see.
    let items = driver.find_one_by("[data-test=items]").await?;
    let html = items.prop("innerHTML").await?;
    ensure!(!html.contains("e2e"), "the markup shows the mark: {html}");

    ensure!(driver.mark("[data-test=nothing]").await? == 0);
    Ok(())
}

pub async fn mark_takes_any_by(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let marked = driver.mark(By::XPath("//li[text()='two']")).await?;
    ensure!(marked == 1, "marked {marked}");

    // Selectors with quotes reach the page as given.
    ensure!(driver.kept_marked("li:not([data-x=\"it's\"])").await? == 1);
    Ok(())
}
