//! `find_one_by`, `find_one_by_xpath`, `find_nth`, `find_first`, `find_all`,
//! `find_all_within`, `find_all_texts`, `rendered_texts`, and the same on an
//! element.

use std::time::Duration;

use yew_e2e::prelude::*;

use crate::{Page, Pages};

pub const PAGE: Page = Page {
    name: "finding",
    body: r##"
<section data-test="box"><span class="x">a</span><span class="x">b</span><span class="x">c</span></section>
<span class="x">outside</span>
<p data-test="solo" id="solo">only</p>
<p data-test="lines">first<br>second</p>
<button data-test="spawn">Spawn</button>
"##,
    script: r##"
document.querySelector("[data-test=spawn]").addEventListener("click", () => {
    setTimeout(() => {
        const late = document.createElement("p");
        late.dataset.test = "late";
        late.textContent = "arrived";
        document.body.prepend(late);
    }, 300);
});
"##,
};

pub async fn find_one_by_answers_the_one_match(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let solo = driver.find_one_by("[data-test=solo]").await?;
    ensure!(solo.text().await? == "only");
    Ok(())
}

pub async fn find_one_by_refuses_several(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver.find_one_by(".x").await else {
        bail!("four matches were taken for one");
    };

    let said = error.to_string();

    ensure!(
        said.contains("Expected exactly one") && said.contains("but got 4"),
        "the refusal said {said:?}"
    );
    ensure!(
        said.contains("find_nth") && said.contains("find_first"),
        "the refusal does not say what to use instead: {said:?}"
    );

    Ok(())
}

pub async fn find_one_by_waits_for_an_element_to_arrive(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.press_nth("[data-test=spawn]", 0).await?;
    let late = driver.find_one_by("[data-test=late]").await?;
    ensure!(late.text().await? == "arrived");
    Ok(())
}

pub async fn find_one_by_xpath_finds_by_xpath(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let solo = driver.find_one_by_xpath("//p[@data-test='solo']").await?;
    ensure!(solo.text().await? == "only");

    let b = driver
        .find_one_by("[data-test=box]")
        .await?
        .find_one_by_xpath("./span[2]")
        .await?;
    ensure!(b.text().await? == "b");
    Ok(())
}

pub async fn find_nth_and_find_first_pick_by_index(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(driver.find_first(".x").await?.text().await? == "a");
    ensure!(driver.find_nth(".x", 1).await?.text().await? == "b");
    ensure!(driver.find_nth(".x", 3).await?.text().await? == "outside");
    Ok(())
}

pub async fn find_nth_gives_up_naming_the_index(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver.find_nth(".x", 9).await else {
        bail!("a tenth match was found among four");
    };

    let said = error.to_string();

    ensure!(
        said.starts_with("Timed out after") && said.contains(".x") && said.ends_with("at 9"),
        "the timeout said {said:?}"
    );

    Ok(())
}

pub async fn find_all_and_count_see_every_match(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(driver.find_all(By::Css(".x")).await?.len() == 4);
    ensure!(driver.find_all(By::Css(".none")).await?.is_empty());
    ensure!(driver.count(".x").await? == 4);
    Ok(())
}

pub async fn find_all_texts_reads_each_match(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let texts = driver.find_all_texts(".x").await?;
    ensure!(texts == ["a", "b", "c", "outside"], "read {texts:?}");
    Ok(())
}

pub async fn find_all_within_bounds_the_call(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let found = driver
        .find_all_within(Duration::from_secs(1), By::ClassName("x"))
        .await?;
    ensure!(found.len() == 4, "found {}", found.len());
    Ok(())
}

pub async fn rendered_texts_reads_in_one_observation(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let texts = driver.rendered_texts("[data-test=box] .x").await?;
    ensure!(texts == ["a", "b", "c"], "read {texts:?}");

    let lines = driver.rendered_texts("[data-test=lines]").await?;
    ensure!(lines == ["first\nsecond"], "read {lines:?}");

    ensure!(driver.rendered_texts(".none").await?.is_empty());
    Ok(())
}

pub async fn element_finds_are_scoped_to_the_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let section = driver.find_one_by("[data-test=box]").await?;

    ensure!(section.find_all(By::Css(".x")).await?.len() == 3);
    ensure!(section.find_all_texts(".x").await? == ["a", "b", "c"]);
    ensure!(section.find_first(".x").await?.text().await? == "a");
    ensure!(section.find_nth(".x", 2).await?.text().await? == "c");
    ensure!(
        section
            .find_all_within(Duration::from_secs(1), By::Css(".x"))
            .await?
            .len()
            == 3
    );

    let Err(error) = section.find_one_by(".x").await else {
        bail!("three matches in the box were taken for one");
    };
    ensure!(error.to_string().contains("but got 3"), "{error:#}");

    Ok(())
}

pub async fn selectors_can_be_built_or_given_as_by(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let built = format!("[data-test={}]", "solo");
    ensure!(driver.find_one_by(&built).await?.text().await? == "only");
    ensure!(driver.find_one_by(By::Id("solo")).await?.text().await? == "only");
    Ok(())
}
