//! `screenshot_png` (of the page, and of the frame `in_frame` entered), `screenshot`, `snapshot`
//! and `downloads`.

use std::path::PathBuf;

use yew_e2e::prelude::*;

use crate::{DOWNLOAD, Page, Pages};

pub const PAGE: Page = Page {
    name: "capture",
    body: r##"
<h1>capture</h1>
<a data-test="download" href="/download.json">Download</a>
"##,
    script: "",
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

pub async fn screenshot_png_is_a_png(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let png = driver.screenshot_png().await?;
    ensure!(
        png.starts_with(PNG),
        "not a PNG: {:?}",
        &png[..png.len().min(8)]
    );
    Ok(())
}

/// The width and height a PNG's header gives.
fn png_size(png: &[u8]) -> Result<(u32, u32)> {
    let header = png.get(16..24).context("a PNG too short for its header")?;
    let width = u32::from_be_bytes(header[..4].try_into()?);
    let height = u32::from_be_bytes(header[4..].try_into()?);
    Ok((width, height))
}

pub async fn screenshot_png_in_a_frame_takes_the_frame(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let scale = crate::eval(driver, "return window.devicePixelRatio")
        .await?
        .as_f64()
        .context("the device pixel ratio")?;
    let whole = png_size(&driver.screenshot_png().await?)?;

    driver.in_frame(390, 600).await?;
    let png = driver.screenshot_png().await?;
    ensure!(png.starts_with(PNG), "not a PNG");

    let framed = png_size(&png)?;
    let expected = ((390.0 * scale).round(), (600.0 * scale).round());
    ensure!(
        (f64::from(framed.0), f64::from(framed.1)) == expected,
        "the frame's screenshot is {framed:?}, not {expected:?}; the whole window's was {whole:?}"
    );

    driver.snapshot("api-capture-frame").await?;

    // The driver is still in the frame.
    ensure!(
        crate::eval(driver, "return window.self !== window.top").await? == true,
        "the screenshot left the frame"
    );
    driver.find_one_by("h1").await?;
    Ok(())
}

pub async fn screenshot_writes_a_file(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shot.png");

    driver.screenshot(&path).await?;
    ensure!(std::fs::read(&path)?.starts_with(PNG), "not a PNG");
    Ok(())
}

pub async fn snapshot_follows_e2e_shots(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    const NAME: &str = "api-capture-snapshot";

    // `E2E_SHOTS` belongs to whoever runs the suite, so this reads it rather
    // than sets it: unset, a snapshot is nothing at all; set, it is a file.
    let Some(dir) = std::env::var_os("E2E_SHOTS") else {
        return driver.snapshot(NAME).await;
    };

    let dir = PathBuf::from(dir);
    let dir = match dir.is_absolute() {
        true => dir,
        false => yew_e2e::target_dir()?.join("e2e-shots").join(dir),
    };
    let path = dir.join(format!("{NAME}.png"));
    let _ = std::fs::remove_file(&path);

    driver.snapshot(NAME).await?;
    ensure!(
        std::fs::read(&path)?.starts_with(PNG),
        "{} is not a PNG",
        path.display()
    );
    Ok(())
}

pub async fn downloads_collects_saved_files(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let downloads = driver.downloads().to_owned();
    ensure!(
        downloads.is_dir(),
        "{} is not a directory",
        downloads.display()
    );
    ensure!(
        std::fs::read_dir(&downloads)?.next().is_none(),
        "the downloads directory did not start empty"
    );

    // Clicked from the page rather than through WebDriver: geckodriver holds a
    // click on a download link for about 5s, waiting for a navigation that
    // never comes.
    crate::eval(
        driver,
        "document.querySelector('[data-test=download]').click()",
    )
    .await?;

    let saved = downloads.join("download.json");

    driver
        .wait_until("the download to be saved", async || {
            Ok(std::fs::read_to_string(&saved).is_ok_and(|saved| saved == DOWNLOAD))
        })
        .await
}
