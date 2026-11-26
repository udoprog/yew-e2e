//! `wait_until`, `wait_until_within`, `wait_texts`, `wait_text`,
//! `wait_text_contains`, `wait_count`, `wait_count_from`, `count`, and the
//! timeouts they wait against.

use std::cell::Cell;
use std::time::{Duration, Instant};

use tokio::time;
use yew_e2e::prelude::*;

use crate::{BRIEF, Page, Pages};

pub const PAGE: Page = Page {
    name: "waits",
    body: r##"
<ul data-test="items"><li>one</li></ul>
<button data-test="add">Add</button>
<p data-test="late" hidden>late</p>
<p data-test="status">idle</p>
<button data-test="work">Work</button>
<p data-test="swap">first</p>
<button data-test="replace">Replace</button>
"##,
    script: r##"
document.querySelector("[data-test=add]").addEventListener("click", () => {
    setTimeout(() => {
        const item = document.createElement("li");
        item.textContent = "two";
        document.querySelector("[data-test=items]").append(item);
    }, 200);
});
setTimeout(() => { document.querySelector("[data-test=late]").hidden = false; }, 300);
document.querySelector("[data-test=work]").addEventListener("click", () => {
    const status = document.querySelector("[data-test=status]");
    setTimeout(() => {
        const replaced = document.createElement("p");
        replaced.dataset.test = "status";
        replaced.textContent = "working: 1 of 2";
        status.replaceWith(replaced);
    }, 100);
    setTimeout(() => { document.querySelector("[data-test=status]").textContent = "done"; }, 300);
});
let replaced = false;
document.querySelector("[data-test=replace]").addEventListener("click", () => {
    if (replaced) { return; }
    replaced = true;
    const swap = document.createElement("p");
    swap.dataset.test = "swap";
    swap.textContent = "second";
    document.querySelector("[data-test=swap]").replaceWith(swap);
});
"##,
};

pub async fn wait_until_polls_until_the_condition_holds(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let polls = Cell::new(0);

    driver
        .wait_until("the third poll", async || {
            polls.set(polls.get() + 1);
            Ok(polls.get() == 3)
        })
        .await?;

    ensure!(polls.get() == 3, "polled {} times", polls.get());

    driver
        .wait_until("the late paragraph", async || {
            driver
                .find_one_by("[data-test=late]")
                .await?
                .visible()
                .await
        })
        .await
}

pub async fn wait_until_within_gives_up_naming_what_it_waited_for(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver
        .wait_until_within(BRIEF, "the moon to rise", async || Ok(false))
        .await
    else {
        bail!("a condition that never holds was waited out");
    };

    let said = error.to_string();

    ensure!(
        said == format!("Timed out after {BRIEF:?} waiting for the moon to rise"),
        "the timeout said {said:?}"
    );

    Ok(())
}

/// A find inside a wait is bounded by what is left of the wait rather than
/// by the find's own timeout, so a page too busy to answer cannot carry a
/// brief wait seconds past its deadline.
pub async fn wait_until_within_bounds_a_slow_find(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    // Keep the page's main thread busy for longer than the wait, so that no
    // find can be answered until it is done. The loop is run by the page
    // itself, since a timer set from a WebDriver script may run elsewhere.
    driver
        .webdriver()
        .execute(
            "const busy = document.createElement('script'); \
             busy.textContent = 'setTimeout(() => { \
                 const end = Date.now() + 3000; while (Date.now() < end) {} }, 50);'; \
             document.body.append(busy);",
            Vec::new(),
        )
        .await?;

    time::sleep(Duration::from_millis(100)).await;

    let start = Instant::now();

    let result = driver
        .wait_until_within(BRIEF, "a busy page to answer", async || {
            Ok(driver
                .find_all(By::Css("[data-test=items] li"))
                .await?
                .len()
                == 2)
        })
        .await;

    let took = start.elapsed();

    // Let the page finish being busy before anything else asks it something.
    driver.wait_count("[data-test=items] li", 1).await?;

    let Err(error) = result else {
        bail!("a page too busy to answer was waited out");
    };

    ensure!(
        error.to_string() == format!("Timed out after {BRIEF:?} waiting for a busy page to answer"),
        "the wait said {error:#}"
    );
    ensure!(
        took < Duration::from_millis(1500),
        "a {BRIEF:?} wait took {took:?}"
    );

    Ok(())
}

pub async fn wait_until_passes_on_the_condition_error(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver
        .wait_until("anything", async || Err(anyhow!("the condition broke")))
        .await
    else {
        bail!("a failing condition was waited out");
    };

    ensure!(
        error.to_string() == "the condition broke",
        "the wait said {error:#}"
    );

    Ok(())
}

pub async fn wait_texts_follows_the_page(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.wait_texts("[data-test=items] li", ["one"]).await?;
    driver.find_one_by("[data-test=add]").await?.click().await?;
    driver
        .wait_texts("[data-test=items] li", ["one", "two"])
        .await
}

pub async fn wait_texts_gives_up_saying_what_it_last_read(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver.wait_texts("[data-test=items] li", ["uno"]).await else {
        bail!("texts that never appear were waited out");
    };

    let said = format!("{error:#}");

    ensure!(
        said.starts_with(r#"last read ["one"]: Timed out after "#),
        "the timeout said {said:?}"
    );
    ensure!(
        said.contains("[data-test=items] li") && said.ends_with(r#"to read ["uno"]"#),
        "the timeout does not say what it waited for: {said:?}"
    );

    Ok(())
}

pub async fn wait_until_looks_again_after_a_stale_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let attempts = Cell::new(0);

    driver
        .wait_until("the swapped paragraph to read second", async || {
            attempts.set(attempts.get() + 1);
            let swap = driver.find_one_by("[data-test=swap]").await?;
            // The first press replaces the paragraph under the handle.
            driver
                .find_one_by("[data-test=replace]")
                .await?
                .click()
                .await?;
            Ok(swap.text().await? == "second")
        })
        .await?;

    ensure!(
        attempts.get() == 2,
        "the stale read took {} attempts",
        attempts.get()
    );
    Ok(())
}

pub async fn wait_text_follows_one_element_through_a_redraw(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.wait_text("[data-test=status]", "idle").await?;
    driver.press_nth("[data-test=work]", 0).await?;
    driver
        .wait_text_contains("[data-test=status]", "1 of 2")
        .await?;
    driver.wait_text("[data-test=status]", "done").await
}

pub async fn wait_text_gives_up_saying_what_it_last_read(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver.wait_text("[data-test=status]", "busy").await else {
        bail!("a text that never appears was waited out");
    };

    let said = format!("{error:#}");

    ensure!(
        said.starts_with(r#"last read "idle": Timed out after "#)
            && said.contains("[data-test=status]")
            && said.ends_with(r#"to read "busy""#),
        "the timeout said {said:?}"
    );

    let Err(error) = driver
        .wait_text_contains("[data-test=nothing]", "busy")
        .await
    else {
        bail!("a text on nothing was waited out");
    };

    let said = format!("{error:#}");

    ensure!(
        said.starts_with("nothing matched to read: Timed out after ")
            && said.ends_with(r#"to contain "busy""#),
        "the timeout said {said:?}"
    );

    Ok(())
}

pub async fn wait_text_refuses_several(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.press_nth("[data-test=add]", 0).await?;
    driver.wait_count("[data-test=items] li", 2).await?;

    let Err(error) = driver.wait_text("[data-test=items] li", "one").await else {
        bail!("one text of several was accepted");
    };

    ensure!(
        error.to_string().contains("but got 2"),
        "the refusal said {error:#}"
    );

    Ok(())
}

pub async fn wait_count_establishes_a_count(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    driver.wait_count("[data-test=items] li", 1).await?;
    ensure!(driver.count("[data-test=items] li").await? == 1);
    ensure!(driver.count("[data-test=nothing]").await? == 0);
    Ok(())
}

pub async fn wait_count_from_follows_a_change(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let before = driver.count("[data-test=items] li").await?;
    driver.press_nth("[data-test=add]", 0).await?;
    driver
        .wait_count_from("[data-test=items] li", before, before + 1)
        .await
}

pub async fn wait_count_from_refuses_a_count_that_did_not_change(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let Err(error) = driver.wait_count_from("[data-test=items] li", 1, 1).await else {
        bail!("an unchanged count was accepted");
    };

    ensure!(
        error.to_string().contains("verifies nothing"),
        "the refusal said {error:#}"
    );

    Ok(())
}

pub async fn element_waits_are_scoped_to_the_element(
    driver: &mut TestDriver,
    pages: &mut Pages,
) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let items = driver.find_one_by("[data-test=items]").await?;

    driver.press_nth("[data-test=add]", 0).await?;
    items.wait_texts("li", ["one", "two"]).await?;
    items.wait_text("li:last-child", "two").await?;
    items.wait_count("li", 2).await?;
    ensure!(items.count("button").await? == 0, "the list has a button");

    items
        .wait_until("the list to hold two", async || {
            Ok(items.count("li").await? == 2)
        })
        .await?;

    let Err(error) = items
        .wait_until_within(BRIEF, "a third item", async || Ok(false))
        .await
    else {
        bail!("an element's wait never gave up");
    };

    ensure!(
        error.to_string().ends_with("waiting for a third item"),
        "the element's timeout said {error:#}"
    );

    Ok(())
}

pub async fn timeouts_are_ordered(driver: &mut TestDriver, pages: &mut Pages) -> Result<()> {
    pages.open(driver, &PAGE).await?;

    let wait = driver.wait_timeout();
    let load = driver.load_timeout();

    ensure!(!wait.is_zero(), "the wait timeout is zero");
    ensure!(
        load >= wait,
        "loading ({load:?}) is given less time than a wait ({wait:?})"
    );

    Ok(())
}
