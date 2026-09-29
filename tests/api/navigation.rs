//! `reload`, `reopen_with`, `in_second_tab`, `in_second_window`,
//! `set_window_size`, `what_the_page_says` and `local_clock`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, eval};

pub const PAGE: Page = Page {
    name: "navigation",
    body: r##"
<output data-test="query"></output>
<output data-test="loads"></output>
<output data-test="bumps">0</output>
<button data-test="bump">Bump</button>
<button data-test="throw">Throw</button>
"##,
    script: r##"
document.querySelector("[data-test=query]").textContent = location.search;
const loads = Number(sessionStorage.getItem("loads") || 0) + 1;
sessionStorage.setItem("loads", String(loads));
document.querySelector("[data-test=loads]").textContent = String(loads);
const bumps = document.querySelector("[data-test=bumps]");
document.querySelector("[data-test=bump]").addEventListener("click", () => {
    bumps.textContent = String(Number(bumps.textContent) + 1);
});
document.querySelector("[data-test=throw]").addEventListener("click", () => {
    setTimeout(() => { throw new Error("thrown in the second tab"); });
});
"##,
};

pub async fn reload_starts_the_page_again(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.wait_texts("[data-test=loads]", ["1"]).await?;
    driver.press_nth("[data-test=bump]", 0).await?;
    driver.wait_texts("[data-test=bumps]", ["1"]).await?;

    driver.reload().await?;
    driver.wait_texts("[data-test=loads]", ["2"]).await?;
    driver.wait_texts("[data-test=bumps]", ["0"]).await?;

    // What the reloaded page throws is still heard.
    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
        "the reloaded page is not watched"
    );
    Ok(())
}

pub async fn reopen_with_sets_the_query(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.wait_texts("[data-test=query]", [""]).await?;
    driver.reopen_with("a=1&b=two").await?;
    driver
        .wait_texts("[data-test=query]", ["?a=1&b=two"])
        .await?;

    let url = driver.webdriver().current_url().await?;
    ensure!(url.path() == "/navigation", "reopened at {url}");
    Ok(())
}

pub async fn in_second_tab_runs_beside_the_first(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let driver = &*driver;
    driver.press_nth("[data-test=bump]", 0).await?;
    let first = driver.webdriver().window().await?;

    driver
        .in_second_tab(async || {
            driver.find_one_by("body[data-page=navigation]").await?;
            driver.wait_texts("[data-test=bumps]", ["0"]).await?;
            ensure!(driver.webdriver().windows().await?.len() == 2);
            ensure!(
                driver.webdriver().window().await? != first,
                "still in the first tab"
            );
            Ok(())
        })
        .await?;

    ensure!(
        driver.webdriver().windows().await?.len() == 1,
        "the tab stayed open"
    );
    ensure!(
        driver.webdriver().window().await? == first,
        "not back in the first tab"
    );
    driver.wait_texts("[data-test=bumps]", ["1"]).await
}

pub async fn in_second_window_runs_beside_the_first(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let driver = &*driver;
    let first = driver.webdriver().window().await?;

    driver
        .in_second_window(async || {
            driver.find_one_by("body[data-page=navigation]").await?;
            ensure!(driver.webdriver().windows().await?.len() == 2);
            Ok(())
        })
        .await?;

    ensure!(
        driver.webdriver().windows().await?.len() == 1,
        "the window stayed open"
    );
    ensure!(
        driver.webdriver().window().await? == first,
        "not back in the first window"
    );
    Ok(())
}

pub async fn in_second_tab_fails_on_what_the_page_threw(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let driver = &*driver;
    let first = driver.webdriver().window().await?;

    let result = driver
        .in_second_tab(async || {
            driver.find_one_by("body[data-page=navigation]").await?;
            driver.watch_for_errors().await?;
            driver.press_nth("[data-test=throw]", 0).await?;
            driver
                .wait_until("the throw to be heard", async || {
                    Ok(driver.error_seen().await?.is_some())
                })
                .await
        })
        .await;

    let Err(error) = result else {
        bail!("what the second tab threw was not heard");
    };

    let said = format!("{error:#}");
    ensure!(
        said.contains("the page threw")
            && (said.contains("thrown in the second tab")
                // Firefox says where, not what; see card 52a92fd9.
                || said.contains(&format!("{}:", pages.address("navigation")))),
        "the second tab's failure said {said:?}"
    );
    ensure!(
        driver.webdriver().windows().await?.len() == 1,
        "the tab stayed open"
    );
    ensure!(
        driver.webdriver().window().await? == first,
        "not back in the first tab"
    );
    Ok(())
}

pub async fn set_window_size_resizes_the_window(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.set_window_size(900, 700).await?;
    let size = eval(driver, "return [window.outerWidth, window.outerHeight]").await?;
    ensure!(
        size == serde_json::json!([900, 700]),
        "the window is {size}"
    );

    driver.set_window_size(1200, 800).await?;
    let size = eval(driver, "return [window.outerWidth, window.outerHeight]").await?;
    ensure!(
        size == serde_json::json!([1200, 800]),
        "the window is {size}"
    );
    Ok(())
}

pub async fn what_the_page_says_describes_the_page(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let said = driver.what_the_page_says().await?;

    ensure!(
        said.contains("the page showing is 'navigation'")
            && said.contains("element(s) are mounted under the body")
            && said.contains("the document is complete"),
        "the page said {said:?}"
    );
    ensure!(!said.contains("threw"), "the page said it threw: {said:?}");
    Ok(())
}

pub async fn local_clock_reads_the_browser_clock(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    // 2024-06-15T12:30:00Z, far enough from a month's ends that no zone's
    // offset moves it out of June.
    const AT: u64 = 1_718_454_600_000;

    let offset = eval(
        driver,
        &format!("return new Date({AT}).getTimezoneOffset()"),
    )
    .await?
    .as_i64()
    .context("a timezone offset")?;

    let minutes = 15 * 24 * 60 + 12 * 60 + 30 - offset;
    let expected = [
        6,
        u32::try_from(minutes / (24 * 60))?,
        u32::try_from(minutes / 60 % 24)?,
        u32::try_from(minutes % 60)?,
    ];

    let clock = driver.local_clock(AT).await?;
    ensure!(
        clock == expected,
        "read {clock:?} at offset {offset}, expected {expected:?}"
    );
    Ok(())
}
