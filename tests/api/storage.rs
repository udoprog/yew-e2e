//! `local_storage`, `set_local_storage` and `block_local_storage`.

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged_exactly};

pub const PAGE: Page = Page {
    name: "storage",
    body: r##"
<button data-test="read">Read</button>
"##,
    script: r##"
document.querySelector("[data-test=read]").addEventListener("click", () => {
    try {
        record(`read ${localStorage.getItem("key")}`);
    } catch (e) {
        record(`blocked ${e.name}`);
    }
});
"##,
};

pub async fn local_storage_round_trips(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    ensure!(driver.local_storage("key").await?.is_none());

    driver.set_local_storage("key", "a \"value\"").await?;
    ensure!(driver.local_storage("key").await?.as_deref() == Some("a \"value\""));

    driver.press_nth("[data-test=read]", 0).await?;
    logged_exactly(driver, ["read a \"value\""]).await
}

pub async fn block_local_storage_denies_the_page(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.block_local_storage().await?;
    driver.press_nth("[data-test=read]", 0).await?;
    logged_exactly(driver, ["blocked SecurityError"]).await
}
