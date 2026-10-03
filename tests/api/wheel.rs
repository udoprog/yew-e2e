//! `wheel`, `wheel_across`, `wheel_alt`, `wheel_over` and `wheel_consumed`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged_exactly};

pub const PAGE: Page = Page {
    name: "wheel",
    body: r##"
<style>
[data-test=taker] { position: absolute; left: 100px; top: 100px; width: 200px; height: 100px; background: #ddd; }
[data-test=free] { position: absolute; left: 400px; top: 100px; width: 200px; height: 100px; background: #eee; }
</style>
<div data-test="taker">taker</div>
<div data-test="free">free</div>
"##,
    script: r##"
const taker = document.querySelector("[data-test=taker]");
taker.addEventListener("wheel", e => {
    e.preventDefault();
    const box = taker.getBoundingClientRect();
    const centred = Math.abs(e.clientX - box.left - box.width / 2) < 1
        && Math.abs(e.clientY - box.top - box.height / 2) < 1;
    const mods = (e.ctrlKey ? " ctrl" : "") + (e.shiftKey ? " shift" : "") + (e.altKey ? " alt" : "");
    record(`wheel ${e.deltaX},${e.deltaY}${mods}${centred ? " centre" : ""}`);
});
"##,
};

pub async fn wheel_turns_down(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let taker = driver.find_one_by("[data-test=taker]").await?;
    driver.wheel(&taker, 120.0).await?;
    logged_exactly(driver, ["wheel 0,120"]).await
}

pub async fn wheel_across_turns_sideways(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let taker = driver.find_one_by("[data-test=taker]").await?;
    driver.wheel_across(&taker, -30.0).await?;
    logged_exactly(driver, ["wheel -30,0"]).await
}

pub async fn wheel_alt_holds_alt_over_the_middle(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let taker = driver.find_one_by("[data-test=taker]").await?;
    driver.wheel_alt(&taker, -50.0, false).await?;
    driver.wheel_alt(&taker, 50.0, true).await?;
    logged_exactly(
        driver,
        ["wheel 0,-50 alt centre", "wheel 0,50 shift alt centre"],
    )
    .await
}

pub async fn wheel_over_says_whether_the_page_took_it(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let taker = driver.find_one_by("[data-test=taker]").await?;
    let free = driver.find_one_by("[data-test=free]").await?;

    ensure!(
        driver.wheel_over(&taker, 5.0, 10.0, true).await?,
        "a wheel the page prevented reads as not taken"
    );
    ensure!(
        !driver.wheel_over(&free, 5.0, 10.0, false).await?,
        "a wheel nothing prevented reads as taken"
    );
    logged_exactly(driver, ["wheel 5,10 ctrl centre"]).await
}

pub async fn wheel_consumed_says_whether_the_page_took_it(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let taker = driver.find_one_by("[data-test=taker]").await?;
    let free = driver.find_one_by("[data-test=free]").await?;

    ensure!(driver.wheel_consumed(&taker, 40.0).await?);
    ensure!(!driver.wheel_consumed(&free, 40.0).await?);
    logged_exactly(driver, ["wheel 0,40"]).await
}

pub async fn wheel_refuses_an_unfinite_delta(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let taker = driver.find_one_by("[data-test=taker]").await?;

    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let refusals = [
            driver.wheel(&taker, bad).await.err(),
            driver.wheel_across(&taker, bad).await.err(),
            driver.wheel_alt(&taker, bad, true).await.err(),
            driver.wheel_consumed(&taker, bad).await.err(),
            driver.wheel_over(&taker, bad, 1.0, false).await.err(),
            driver.wheel_over(&taker, 1.0, bad, false).await.err(),
        ];

        for (index, refusal) in refusals.into_iter().enumerate() {
            let Some(error) = refusal else {
                bail!("call {index} accepted a delta of {bad}");
            };

            ensure!(
                format!("{error:#}").contains("must be finite"),
                "call {index} said {error:#}"
            );
        }
    }

    // Nothing reached the page: the first wheel it hears is this one.
    driver.wheel(&taker, 1.0).await?;
    logged_exactly(driver, ["wheel 0,1"]).await
}
