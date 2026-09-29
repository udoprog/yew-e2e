//! Presses (`click`, `extend_click`, `shift_click`, `click_by`,
//! `double_click`, `double_click_by`, `context_click`, `hover`,
//! `middle_click`, `press_nth`, `context_press_nth`) and held drags
//! (`drag_by`, `drag_from_by`, `alt_drag_by`, `move_held`, `drop_held`,
//! `drop_held_alt`, `cancel_pointer`, `release_all_input`, `middle_drag_by`,
//! `pointer_drag_onto`).

use yew_e2e::prelude::*;

use crate::{Page, Pages, logged, logged_exactly};

pub const PAGE: Page = Page {
    name: "pointer",
    body: r##"
<style>
[data-test=pad] { position: absolute; left: 100px; top: 100px; width: 200px; height: 100px; background: #ddd; }
[data-test=other] { position: absolute; left: 400px; top: 100px; width: 100px; height: 100px; background: #bbd; }
[data-test=items] { position: absolute; left: 600px; top: 100px; }
[data-test=surface] { position: absolute; left: 100px; top: 300px; width: 300px; height: 200px; background: #dbd; touch-action: none; user-select: none; }
</style>
<div data-test="pad">pad</div>
<div data-test="other">other</div>
<div data-test="items"><button data-test="item">one</button> <button data-test="item">two</button></div>
<div data-test="surface">surface</div>
<output data-test="held"></output>
"##,
    script: r##"
const from = (e, node) => {
    const box = node.getBoundingClientRect();
    return `${Math.round(e.clientX - box.left - box.width / 2)},${Math.round(e.clientY - box.top - box.height / 2)}`;
};
const mods = e => (e.ctrlKey ? " ctrl" : "") + (e.shiftKey ? " shift" : "") + (e.altKey ? " alt" : "");

const pad = document.querySelector("[data-test=pad]");
for (const type of ["click", "dblclick", "contextmenu"]) {
    pad.addEventListener(type, e => {
        if (type === "contextmenu") e.preventDefault();
        record(`${type} ${from(e, pad)}${mods(e)}`);
    });
}
pad.addEventListener("auxclick", e => record(`auxclick b${e.button}`));

const other = document.querySelector("[data-test=other]");
other.addEventListener("mouseenter", () => record("enter other"));
other.addEventListener("pointerup", () => record("up on other"));

for (const item of document.querySelectorAll("[data-test=item]")) {
    item.addEventListener("click", () => record(`pressed ${item.textContent}`));
    item.addEventListener("contextmenu", e => { e.preventDefault(); record(`context ${item.textContent}`); });
}

const surface = document.querySelector("[data-test=surface]");
const held = document.querySelector("[data-test=held]");
const button = e => e.button ? ` b${e.button}` : "";
surface.addEventListener("pointerdown", e => {
    surface.setPointerCapture(e.pointerId);
    record(`down ${from(e, surface)}${button(e)}${mods(e)}`);
});
surface.addEventListener("pointermove", e => { held.textContent = `${e.buttons} ${from(e, surface)}`; });
surface.addEventListener("pointerup", e => record(`up ${from(e, surface)}${button(e)}${mods(e)}`));
surface.addEventListener("pointercancel", () => record("cancel"));
"##,
};

/// Where a held drag of `offset` along one axis comes to rest, from where it
/// was pressed.
///
/// `drag_by`, `drag_from_by` and `alt_drag_by` say they drag `dx`, `dy`, but
/// they first step 8px towards it and then move the whole of `dx`, `dy` on
/// top, so they land 8px past it on every axis that moves; see card
/// 4a1c22ae. These tests pin what they do today, so a fix changes this
/// one function.
fn landed(offset: i64) -> i64 {
    offset + offset.signum() * 8
}

/// Where the surface's last move had the pointer, and which buttons it held.
async fn held(driver: &TestDriver, expected: &str) -> Result<()> {
    driver.wait_texts("[data-test=held]", [expected]).await
}

pub async fn click_presses_the_centre(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.find_one_by("[data-test=pad]").await?.click().await?;
    logged_exactly(driver, ["click 0,0"]).await
}

pub async fn extend_click_holds_control(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=pad]")
        .await?
        .extend_click()
        .await?;
    logged_exactly(driver, ["click 0,0 ctrl"]).await
}

pub async fn shift_click_holds_shift(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=pad]")
        .await?
        .shift_click()
        .await?;
    logged_exactly(driver, ["click 0,0 shift"]).await
}

pub async fn click_by_presses_off_centre(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=pad]")
        .await?
        .click_by(50, -20)
        .await?;
    logged_exactly(driver, ["click 50,-20"]).await
}

pub async fn double_click_is_one_gesture(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=pad]")
        .await?
        .double_click()
        .await?;
    logged_exactly(driver, ["click 0,0", "click 0,0", "dblclick 0,0"]).await
}

pub async fn double_click_by_is_off_centre(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=pad]")
        .await?
        .double_click_by(-40, 10)
        .await?;
    logged_exactly(driver, ["click -40,10", "click -40,10", "dblclick -40,10"]).await
}

pub async fn context_click_opens_the_context_menu(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=pad]")
        .await?
        .context_click()
        .await?;
    logged(driver, "contextmenu 0,0").await?;

    let log = crate::log(driver).await?;
    ensure!(
        !log.iter().any(|line| line.starts_with("click")),
        "a right-click also clicked: {log:?}"
    );
    Ok(())
}

pub async fn hover_rests_the_pointer(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver
        .find_one_by("[data-test=other]")
        .await?
        .hover()
        .await?;
    logged_exactly(driver, ["enter other"]).await
}

pub async fn middle_click_presses_the_middle_button(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let pad = driver.find_one_by("[data-test=pad]").await?;
    driver.middle_click(&pad).await?;
    logged_exactly(driver, ["auxclick b1"]).await
}

pub async fn press_nth_presses_by_index(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.press_nth("[data-test=item]", 1).await?;
    driver.press_nth("[data-test=item]", 0).await?;
    logged_exactly(driver, ["pressed two", "pressed one"]).await
}

pub async fn context_press_nth_right_clicks_by_index(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.context_press_nth("[data-test=item]", 1).await?;
    logged_exactly(driver, ["context two"]).await
}

pub async fn drag_by_holds_the_button_until_drop_held(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    surface.drag_by(60, 20).await?;

    logged_exactly(driver, ["down 0,0"]).await?;
    held(driver, &format!("1 {},{}", landed(60), landed(20))).await?;

    driver.drop_held().await?;
    let up = format!("up {},{}", landed(60), landed(20));
    logged_exactly(driver, ["down 0,0", &up]).await
}

pub async fn drag_from_by_begins_off_centre(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    surface.drag_from_by(-50, 10, -30, 0).await?;
    held(driver, &format!("1 {},10", -50 + landed(-30))).await?;

    driver.drop_held().await?;
    let up = format!("up {},10", -50 + landed(-30));
    logged_exactly(driver, ["down -50,10", &up]).await
}

pub async fn move_held_carries_the_held_drag(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    surface.drag_by(40, 0).await?;
    held(driver, &format!("1 {},0", landed(40))).await?;

    driver.move_held(0, 30).await?;
    held(driver, &format!("1 {},30", landed(40))).await?;

    driver.drop_held().await?;
    let up = format!("up {},30", landed(40));
    logged_exactly(driver, ["down 0,0", &up]).await
}

pub async fn alt_drag_by_holds_alt_until_drop_held_alt(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    surface.alt_drag_by(0, 50).await?;
    logged_exactly(driver, ["down 0,0 alt"]).await?;

    driver.drop_held_alt().await?;
    let up = format!("up 0,{} alt", landed(50));
    logged_exactly(driver, ["down 0,0 alt", &up]).await?;

    // Alt was given back along with the button.
    surface.click().await?;
    logged(driver, "up 0,0").await
}

pub async fn cancel_pointer_cancels_without_releasing(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    surface.drag_by(20, 0).await?;

    driver.cancel_pointer(&surface).await?;
    logged_exactly(driver, ["down 0,0", "cancel"]).await?;

    driver.move_held(10, 0).await?;
    held(driver, &format!("1 {},0", landed(20) + 10)).await?;

    driver.drop_held().await?;
    let up = format!("up {},0", landed(20) + 10);
    logged_exactly(driver, ["down 0,0", "cancel", &up]).await
}

pub async fn release_all_input_lets_go_of_a_drag(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    surface.drag_by(-20, 0).await?;
    logged_exactly(driver, ["down 0,0"]).await?;

    driver.release_all_input().await?;
    let up = format!("up {},0", landed(-20));
    logged_exactly(driver, ["down 0,0", &up]).await
}

pub async fn middle_drag_by_pans_with_the_middle_button(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let surface = driver.find_one_by("[data-test=surface]").await?;
    driver.middle_drag_by(&surface, 40).await?;
    logged_exactly(driver, ["down 0,0 b1", "up 40,0 b1"]).await
}

pub async fn pointer_drag_onto_releases_over_the_target(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let pad = driver.find_one_by("[data-test=pad]").await?;
    let other = driver.find_one_by("[data-test=other]").await?;

    driver.pointer_drag_onto(&pad, &other).await?;
    logged(driver, "up on other").await
}
