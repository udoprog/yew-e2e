//! HTML drag and drop: `drag_onto`, `drag_over`, `drag_off`,
//! `external_files`, `drag_external_files`, `native_drag_over` and
//! `native_release`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged, logged_exactly};

pub const PAGE: Page = Page {
    name: "drag",
    body: r##"
<style>
[data-test=card] { position: absolute; left: 100px; top: 100px; width: 100px; height: 60px; background: #ddf; }
[data-test=bin] { position: absolute; left: 400px; top: 100px; width: 200px; height: 200px; background: #fdd; }
</style>
<div data-test="card" draggable="true">card</div>
<div data-test="bin">bin</div>
"##,
    script: r##"
const card = document.querySelector("[data-test=card]");
const bin = document.querySelector("[data-test=bin]");
card.addEventListener("dragstart", e => { e.dataTransfer.setData("text/plain", "card"); record("dragstart card"); });
card.addEventListener("dragend", () => record("dragend card"));
bin.addEventListener("dragenter", e => { e.preventDefault(); record("dragenter bin"); });
bin.addEventListener("dragover", e => { e.preventDefault(); if (!bin.dataset.over) { bin.dataset.over = "1"; record("dragover bin"); } });
bin.addEventListener("dragleave", () => { delete bin.dataset.over; record("dragleave bin"); });
bin.addEventListener("drop", e => {
    e.preventDefault();
    delete bin.dataset.over;
    const files = Array.from(e.dataTransfer.files, file => `${file.name}:${file.size}`);
    record(files.length ? `drop bin ${files.join(",")}` : "drop bin");
});
bin.addEventListener("pointerover", () => { if (!bin.dataset.pointer) { bin.dataset.pointer = "1"; record("pointer over bin"); } });
"##,
};

pub async fn drag_onto_runs_the_whole_html_drag(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let card = driver.find_one_by("[data-test=card]").await?;
    let bin = driver.find_one_by("[data-test=bin]").await?;

    driver.drag_onto(&card, &bin).await?;
    logged_exactly(
        driver,
        ["dragstart card", "dragover bin", "drop bin", "dragend card"],
    )
    .await
}

pub async fn drag_over_stops_short_of_the_drop(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let card = driver.find_one_by("[data-test=card]").await?;
    let bin = driver.find_one_by("[data-test=bin]").await?;

    driver.drag_over(&card, &bin).await?;
    logged_exactly(driver, ["dragstart card", "dragover bin"]).await
}

pub async fn drag_off_leaves_the_target(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let card = driver.find_one_by("[data-test=card]").await?;
    let bin = driver.find_one_by("[data-test=bin]").await?;

    driver.drag_over(&card, &bin).await?;
    driver.drag_off(&bin).await?;
    logged_exactly(driver, ["dragstart card", "dragover bin", "dragleave bin"]).await
}

pub async fn external_files_drop_onto_a_target(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let dir = tempfile::tempdir()?;
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.json");
    std::fs::write(&a, "four")?;
    std::fs::write(&b, "{}")?;

    let paths = format!("{}\n{}", a.display(), b.display());
    let files = driver.external_files(&paths).await?;
    ensure!(files.attr("type").await? == "file", "not a file input");

    let bin = driver.find_one_by("[data-test=bin]").await?;

    ensure!(
        driver.drag_external_files(&files, &bin, "dragover").await?,
        "the bin did not take the files it was offered"
    );
    ensure!(
        driver.drag_external_files(&files, &bin, "drop").await?,
        "the bin did not take the files dropped on it"
    );
    logged(driver, "drop bin a.txt:4,b.json:2").await?;

    let card = driver.find_one_by("[data-test=card]").await?;
    ensure!(
        !driver.drag_external_files(&files, &card, "drop").await?,
        "the card, which takes no drop, took one"
    );
    Ok(())
}

pub async fn native_drag_over_carries_with_the_pointer(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let card = driver.find_one_by("[data-test=card]").await?;
    let bin = driver.find_one_by("[data-test=bin]").await?;

    driver.native_drag_over(&card, &bin).await?;
    logged(driver, "dragstart card").await?;
    logged(driver, "dragover bin").await?;

    // Firefox begins a drag for the pointer but never ends one: no `drop`
    // and no `dragend` follow a release, whether this one or a bare
    // WebDriver release action, so the letting go is only asked to succeed.
    driver.native_release().await?;
    driver.release_all_input().await
}
