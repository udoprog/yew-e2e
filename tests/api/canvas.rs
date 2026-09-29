//! `canvas_size`, `canvas_columns` and `drawn_inside`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, near};

pub const PAGE: Page = Page {
    name: "canvas",
    body: r##"
<style>
[data-test=canvas] { position: absolute; left: 20px; top: 20px; width: 200px; height: 100px; }
[data-test=host] { position: absolute; left: 20px; top: 300px; width: 100px; height: 100px; background: #ddd; }
[data-test=bubble] { position: absolute; left: 60px; top: 300px; width: 20px; height: 20px; background: #f00; pointer-events: none; }
</style>
<canvas data-test="canvas" width="200" height="100"></canvas>
<div data-test="host"><span>inside</span></div>
<div data-test="bubble"></div>
"##,
    script: r##"
const context = document.querySelector("[data-test=canvas]").getContext("2d");
context.fillStyle = "#000";
context.fillRect(50, 25, 100, 50);
"##,
};

pub async fn canvas_size_reads_the_three_sizes(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let canvas = driver.find_one_by("[data-test=canvas]").await?;
    let size = driver.canvas_size(&canvas).await?;

    near("device width", size.device_width, 200.0, 0.0)?;
    near("device height", size.device_height, 100.0, 0.0)?;
    near("css width", size.css_width, 200.0, 0.01)?;
    near("css height", size.css_height, 100.0, 0.01)?;
    ensure!(size.device_pixel_ratio > 0.0, "{size:?}");
    Ok(())
}

pub async fn canvas_columns_scans_what_is_drawn(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let canvas = driver.find_one_by("[data-test=canvas]").await?;
    let columns = driver.canvas_columns(&canvas, 4).await?;

    ensure!(
        columns
            == [
                (0.0, -1.0, -1.0),
                (0.25, 0.25, 0.74),
                (0.5, 0.25, 0.74),
                (0.75, -1.0, -1.0),
            ],
        "scanned {columns:?}"
    );
    Ok(())
}

pub async fn drawn_inside_sees_what_stands_over_a_point(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(
        driver.drawn_inside("[data-test=host]", 30.0, 380.0).await?,
        "the host's own point is not drawn inside it"
    );
    ensure!(
        !driver.drawn_inside("[data-test=host]", 70.0, 310.0).await?,
        "a bubble that takes no pointer events was looked through"
    );
    ensure!(
        !driver
            .drawn_inside("[data-test=host]", 500.0, 500.0)
            .await?,
        "a point outside the host is drawn inside it"
    );
    ensure!(
        !driver
            .drawn_inside("[data-test=nothing]", 30.0, 380.0)
            .await?,
        "a host that is not there has something drawn in it"
    );
    Ok(())
}
