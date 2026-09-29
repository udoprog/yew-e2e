//! `set_style`, `set_class`, `computed`, `animation_time`, `reduced_motion`,
//! `painted`, `color_alpha`, `color_shift`, `overflowing` and `small_text`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, eval, near};

pub const PAGE: Page = Page {
    name: "style",
    body: r##"
<style>
[data-test=quote]::before { content: "\201C"; }
@keyframes grow { from { width: 0px; } to { width: 100px; } }
[data-test=anim] { animation: grow 1000ms linear infinite; height: 10px; background: #000; }
[data-test=red] { color: rgb(255, 0, 0); background-color: rgba(0, 0, 0, 0.5); border-top-color: rgba(255, 255, 255, 0.5); }
[data-test=clip] { width: 50px; height: 20px; overflow: hidden; white-space: nowrap; }
[data-test=fits] { width: 400px; }
.tiny, .mark { font-size: 8px; }
.loud { font-weight: 700; }
</style>
<p data-test="quote">q</p>
<div data-test="anim"></div>
<div data-test="red">red</div>
<div data-test="clipbox"><div data-test="clip">a line of text that is far too long for its box</div></div>
<div data-test="fits">fits</div>
<div data-test="texts"><span class="tiny">tiny words</span> <span>normal words</span> <span class="mark">3</span> <span class="tiny" style="visibility: hidden">unseen</span></div>
"##,
    script: "",
};

pub async fn set_style_and_set_class_force_styles(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let fits = driver.find_one_by("[data-test=fits]").await?;

    driver.set_style(&fits, "width", "123px").await?;
    ensure!(fits.css("width").await? == "123px");

    driver.set_class(&fits, "loud", true).await?;
    ensure!(fits.attr("class").await? == "loud");
    ensure!(fits.css("font-weight").await? == "700");

    driver.set_class(&fits, "loud", false).await?;
    ensure!(fits.attr("class").await?.is_empty());
    ensure!(fits.css("font-weight").await? == "400");
    Ok(())
}

pub async fn computed_reads_a_pseudo_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let quote = driver.find_one_by("[data-test=quote]").await?;

    let before = driver.computed(&quote, "::before", "content").await?;
    ensure!(before == "\"\u{201c}\"", "::before is {before:?}");

    let own = driver.computed(&quote, "", "content").await?;
    ensure!(own == "normal", "the element's own content is {own:?}");
    Ok(())
}

pub async fn animation_time_pauses_an_animation(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let anim = driver.find_one_by("[data-test=anim]").await?;

    driver.animation_time(&anim, "grow", 250.0).await?;
    ensure!(anim.css("width").await? == "25px");

    driver.animation_time(&anim, "grow", 500.0).await?;
    ensure!(anim.css("width").await? == "50px");
    Ok(())
}

pub async fn animation_time_refuses_a_missing_animation(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let anim = driver.find_one_by("[data-test=anim]").await?;

    let Err(error) = driver.animation_time(&anim, "shrink", 0.0).await else {
        bail!("an animation that is not there was paused");
    };

    ensure!(
        format!("{error:#}").contains("Missing animation: shrink"),
        "the refusal said {error:#}"
    );
    Ok(())
}

pub async fn reduced_motion_reads_the_media_query(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let page = eval(
        driver,
        "return matchMedia('(prefers-reduced-motion: reduce)').matches",
    )
    .await?;

    let reduced = driver.reduced_motion().await?;
    ensure!(
        page == reduced,
        "the page says {page}, reduced_motion {reduced}"
    );

    if let Ok(asked) = std::env::var("E2E_MOTION") {
        ensure!(
            reduced == (asked == "reduce"),
            "E2E_MOTION={asked} was not honoured"
        );
    }

    Ok(())
}

pub async fn painted_color_alpha_and_color_shift_measure_colour(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let red = driver.find_one_by("[data-test=red]").await?;

    ensure!(driver.painted(&red, "color").await? == [255.0, 0.0, 0.0]);
    near(
        "color alpha",
        driver.color_alpha(&red, "color").await?,
        1.0,
        0.01,
    )?;

    let wash = driver.painted(&red, "background-color").await?;
    for channel in wash {
        near("half black over grey", channel, 64.0, 1.5)?;
    }

    near(
        "background alpha",
        driver.color_alpha(&red, "background-color").await?,
        0.5,
        0.01,
    )?;
    near(
        "black darkens",
        driver.color_shift(&red, "background-color").await?,
        -64.0,
        1.5,
    )?;
    near(
        "white lightens",
        driver.color_shift(&red, "border-top-color").await?,
        64.0,
        1.5,
    )?;

    let fits = driver.find_one_by("[data-test=fits]").await?;
    near(
        "transparent paints nothing",
        driver.color_shift(&fits, "background-color").await?,
        0.0,
        0.5,
    )?;
    near(
        "transparent has no alpha",
        driver.color_alpha(&fits, "background-color").await?,
        0.0,
        0.01,
    )
}

pub async fn overflowing_finds_clipped_boxes(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let clipbox = driver.find_one_by("[data-test=clipbox]").await?;
    let found = driver.overflowing(&clipbox).await?;

    ensure!(found.len() == 1, "found {found:?}");
    ensure!(
        found[0].starts_with("div[data-test=clip] hidden/hidden over by "),
        "found {found:?}"
    );

    let fits = driver.find_one_by("[data-test=fits]").await?;
    ensure!(driver.overflowing(&fits).await?.is_empty());
    Ok(())
}

pub async fn small_text_finds_small_text(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let texts = driver.find_one_by("[data-test=texts]").await?;
    let found = driver.small_text(&texts, 10.0, ".mark").await?;

    ensure!(
        found == [r#"span.tiny at 8px: "tiny words""#],
        "found {found:?}"
    );

    ensure!(driver.small_text(&texts, 6.0, ".mark").await?.is_empty());
    Ok(())
}
