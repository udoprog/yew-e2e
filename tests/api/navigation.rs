//! `reload`, `reopen_with`, `in_second_tab`, `in_second_window` (and the error watch across all
//! four), `wait_page`, `visit`, `wait_window`, `in_new_tab`, `in_new_window`, `set_window_size`,
//! `what_the_page_says` and `local_clock`.

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
<div data-window="it's" hidden>Late</div>
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
setTimeout(() => { document.querySelector("[data-window=\"it's\"]").hidden = false; }, 300);
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

pub async fn reopen_with_keeps_the_watch(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.reopen_with("again=1").await?;
    driver.wait_texts("[data-test=query]", ["?again=1"]).await?;

    // What the reopened page throws is still heard.
    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
        "the reopened page is not watched"
    );
    Ok(())
}

/// A test that never asked to watch the page is not made to by a reload, so
/// what the reloaded page says on its console does not fail it.
pub async fn reload_leaves_an_unwatched_test_unwatched(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    driver.webdriver().goto(pages.address(PAGE.name)).await?;
    driver.find_one_by("body[data-page=navigation]").await?;

    driver.reload().await?;
    driver.wait_texts("[data-test=loads]", ["2"]).await?;

    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == false,
        "the reloaded page is watched"
    );

    eval(
        driver,
        "console.error('said on an unwatched page'); return null",
    )
    .await?;
    ensure!(
        driver.error_seen().await?.is_none(),
        "an unwatched page's console was heard"
    );
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
            // The test is watching, so the second tab is watched too.
            ensure!(
                eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
                "the second tab is not watched"
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
            ensure!(
                eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
                "the second window is not watched"
            );
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

/// The test is watching from [`Pages::open`], so the second tab is watched
/// without the test asking again, and what it throws fails `in_second_tab`.
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
        said.contains("the page threw") && said.contains("thrown in the second tab"),
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

/// A test that never asked to watch the page is not made to by a second tab,
/// so what the page there says on its console does not fail it.
pub async fn in_second_tab_leaves_an_unwatched_test_unwatched(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    driver.webdriver().goto(pages.address(PAGE.name)).await?;
    driver.find_one_by("body[data-page=navigation]").await?;

    let driver = &*driver;

    driver
        .in_second_tab(async || {
            driver.find_one_by("body[data-page=navigation]").await?;
            ensure!(
                eval(driver, "return document.body.hasAttribute('data-watching')").await? == false,
                "the second tab is watched"
            );
            eval(
                driver,
                "console.error('said in an unwatched tab'); return null",
            )
            .await?;
            Ok(())
        })
        .await
        .context("an unwatched second tab failed the test")?;

    ensure!(
        driver.webdriver().windows().await?.len() == 1,
        "the tab stayed open"
    );
    Ok(())
}

pub async fn wait_page_waits_and_watches(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    driver.webdriver().goto(pages.address(PAGE.name)).await?;
    driver.wait_page("navigation").await?;

    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
        "the page is not watched"
    );
    Ok(())
}

pub async fn wait_page_gives_up_saying_what_the_page_says(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    driver.webdriver().goto(pages.address(PAGE.name)).await?;

    let Err(error) = driver.wait_page("elsewhere").await else {
        bail!("waited out a page that never came");
    };

    let said = format!("{error:#}");
    ensure!(
        said.contains("page elsewhere") && said.contains("the page showing is 'navigation'"),
        "the wait said {said:?}"
    );
    Ok(())
}

pub async fn visit_goes_and_returns_where_it_was(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let first = pages.address(PAGE.name);
    let was = driver
        .visit(&format!("{first}?visited=1"), "navigation")
        .await?;

    ensure!(was == first, "came from {was:?}, not {first:?}");
    driver.wait_texts("[data-test=query]", ["?visited=1"]).await
}

pub async fn wait_window_waits_for_a_visible_window(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    driver.webdriver().goto(pages.address(PAGE.name)).await?;
    driver.wait_window("it's").await?;

    ensure!(
        driver
            .find_one_by("[data-window=\"it's\"]")
            .await?
            .visible()
            .await?,
        "the window is not visible"
    );
    ensure!(
        eval(driver, "return document.body.hasAttribute('data-watching')").await? == true,
        "the page is not watched"
    );
    Ok(())
}

pub async fn in_new_tab_and_window_wait_for_the_window(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let driver = &*driver;
    let first = driver.webdriver().window().await?;
    let shown = async || {
        ensure!(driver.webdriver().windows().await?.len() == 2);
        ensure!(
            driver
                .find_one_by("[data-window=\"it's\"]")
                .await?
                .visible()
                .await?,
            "ran before the window was shown"
        );
        Ok(())
    };

    driver.in_new_tab("it's", shown).await?;
    driver.in_new_window("it's", shown).await?;

    ensure!(
        driver.webdriver().windows().await?.len() == 1,
        "a second context stayed open"
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
