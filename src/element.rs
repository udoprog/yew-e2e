//! [`TestElement`], one element a lookup found.

use anyhow::Result;
use anyhow::ensure;
use thirtyfour::common::types::ElementRect;
use thirtyfour::prelude::*;
use tokio::time::{self, Duration};

use crate::finding::common;
use crate::{Holding, IntoBy, Waiting};

/// The first step of a drag along one axis: up to 8px towards `offset`, never
/// past it, so that the rest of the drag moves `offset` less this.
pub(crate) fn drag_nudge(offset: i64) -> i64 {
    offset.clamp(-8, 8)
}

/// A wrapped WebElement.
pub struct TestElement {
    pub(crate) inner: WebElement,
    pub(crate) wait_timeout: Duration,
    pub(crate) waiting: Waiting,
    pub(crate) holding: Holding,
}

impl TestElement {
    /// The WebDriver element underneath, for what this wrapper has no method
    /// for.
    pub fn element(&self) -> &WebElement {
        &self.inner
    }

    /// Click the element.
    pub async fn click(&self) -> Result<()> {
        tracing::trace!("Clicking element");
        self.inner.click().await?;
        Ok(())
    }

    /// **Press the element with control held**, which is how a selection is
    /// commonly extended by one item.
    pub async fn extend_click(&self) -> Result<()> {
        tracing::trace!("Extend-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .key_down(thirtyfour::Key::Control)
            .click_element(&self.inner)
            .key_up(thirtyfour::Key::Control)
            .perform()
            .await?;

        Ok(())
    }

    /// **Press the element with shift held**, which picks the run from the
    /// last item picked to this one in a list that has an order.
    pub async fn shift_click(&self) -> Result<()> {
        tracing::trace!("Shift-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .key_down(thirtyfour::Key::Shift)
            .click_element(&self.inner)
            .key_up(thirtyfour::Key::Shift)
            .perform()
            .await?;

        Ok(())
    }

    /// Rest the pointer on the element without pressing anything.
    pub async fn hover(&self) -> Result<()> {
        tracing::trace!("Hovering element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .perform()
            .await?;

        Ok(())
    }

    /// Press the element twice, as one gesture.
    pub async fn double_click(&self) -> Result<()> {
        tracing::trace!("Double-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .double_click_element(&self.inner)
            .perform()
            .await?;

        Ok(())
    }

    /// **The same, a given offset from the element's centre**, for one element
    /// covering a whole surface, where on it a gesture lands being what the
    /// gesture is about.
    pub async fn double_click_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Double-clicking element off centre");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .move_by_offset(dx, dy)
            .double_click()
            .perform()
            .await?;

        Ok(())
    }

    /// **Press the element once, a given offset from its centre.** A slider is
    /// one element covering its whole travel, so where on it a press lands is
    /// the whole of what the press says.
    pub async fn click_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Clicking element off centre");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .move_by_offset(dx, dy)
            .click()
            .perform()
            .await?;

        Ok(())
    }

    /// Right-click the element.
    pub async fn context_click(&self) -> Result<()> {
        tracing::trace!("Context-clicking element");

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .context_click_element(&self.inner)
            .perform()
            .await?;

        Ok(())
    }

    /// Press on this element and drag `dx`, `dy` from where it is, **leaving
    /// the button down**.
    ///
    /// The pointer first steps up to 8px towards the offset, so the page sees
    /// a drag begin, and then moves the rest of the way: it comes to rest
    /// exactly `dx`, `dy` from where it was pressed.
    pub async fn drag_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Dragging element");

        let (nx, ny) = (drag_nudge(dx), drag_nudge(dy));

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .click_and_hold()
            .move_by_offset(nx, ny)
            .move_by_offset(dx - nx, dy - ny)
            .perform()
            .await?;
        self.holding.set(true);

        Ok(())
    }

    /// The same begun at a point of the element's own choosing rather than at
    /// its centre, `at_x` and `at_y` being pixels from that centre.
    ///
    /// A sweep that has to begin in one row of a grid cannot begin wherever the
    /// element happens to be halved.
    pub async fn drag_from_by(&self, at_x: i64, at_y: i64, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(at_x, at_y, dx, dy, "Dragging from a point in an element");

        let (nx, ny) = (drag_nudge(dx), drag_nudge(dy));

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .move_to_element_center(&self.inner)
            .move_by_offset(at_x, at_y)
            .click_and_hold()
            .move_by_offset(nx, ny)
            .move_by_offset(dx - nx, dy - ny)
            .perform()
            .await?;
        self.holding.set(true);

        Ok(())
    }

    /// The same with `alt` held.
    pub async fn alt_drag_by(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Dragging element with alt held");

        let (nx, ny) = (drag_nudge(dx), drag_nudge(dy));

        self.inner.scroll_into_view().await?;

        self.inner
            .handle()
            .action_chain()
            .key_down(thirtyfour::Key::Alt)
            .move_to_element_center(&self.inner)
            .click_and_hold()
            .move_by_offset(nx, ny)
            .move_by_offset(dx - nx, dy - ny)
            .perform()
            .await?;
        self.holding.set(true);

        Ok(())
    }

    /// Get the text contents for this element.
    pub async fn text(&self) -> Result<String> {
        tracing::trace!("Getting element text");
        Ok(self.inner.text().await?)
    }

    /// One of the element's DOM properties.
    pub async fn prop(&self, name: &str) -> Result<String> {
        tracing::trace!(name, "Getting element property");
        Ok(self.inner.prop(name).await?.unwrap_or_default())
    }

    /// One of the element's DOM *attributes*, or the empty string when it has
    /// none.
    pub async fn attr(&self, name: &str) -> Result<String> {
        tracing::trace!(name, "Getting element attribute");
        Ok(self.inner.attr(name).await?.unwrap_or_default())
    }

    /// Bring the element into view.
    pub async fn scroll_into_view(&self) -> Result<()> {
        tracing::trace!("Scrolling element into view");
        self.inner.scroll_into_view().await?;
        Ok(())
    }

    /// Put keyboard focus here without pressing anything.
    pub async fn focus(&self) -> Result<()> {
        tracing::trace!("Focusing element");
        self.inner.focus().await?;
        Ok(())
    }

    /// Whether the element is displayed, as WebDriver judges it: present in
    /// the page is not enough, it must also not be hidden by CSS
    /// (`display: none`, `visibility: hidden`) or have no size.
    ///
    /// Answers once, at the time of asking; it does not wait.
    pub async fn visible(&self) -> Result<bool> {
        Ok(self.inner.is_displayed().await?)
    }

    /// Where this element is on screen, and how big it is.
    pub async fn rect(&self) -> Result<ElementRect> {
        tracing::trace!("Measuring element");
        Ok(self.inner.rect().await?)
    }

    /// One of the element's *computed* CSS properties, as `getComputedStyle`
    /// serializes it.
    ///
    /// The value is read in the page rather than through WebDriver's own
    /// command, which chromedriver answers with every colour rewritten as
    /// `rgba(...)`: a colour reads `rgb(255, 0, 0)` in every engine alike.
    pub async fn css(&self, name: &str) -> Result<String> {
        tracing::trace!(name, "Getting element style");

        let script = "return getComputedStyle(arguments[0]).getPropertyValue(arguments[1]);";

        let value = self
            .inner
            .handle()
            .execute(script, vec![self.inner.to_json()?, serde_json::json!(name)])
            .await?;

        Ok(value.json().as_str().unwrap_or_default().to_string())
    }

    /// The element's `value` property.
    pub async fn value(&self) -> Result<String> {
        tracing::trace!("Getting element value");
        Ok(self.inner.prop("value").await?.unwrap_or_default())
    }

    /// Clear the element contents.
    pub async fn clear(&self) -> Result<()> {
        tracing::trace!("Clearing element text");
        self.inner.clear().await?;
        Ok(())
    }

    /// Send the specified input targeting the element.
    pub async fn send_keys(&self, keys: &str) -> Result<()> {
        tracing::trace!(?keys, "Sending keys to element");
        self.inner.send_keys(keys).await?;
        Ok(())
    }

    /// Whether the element is currently pressed, by `aria-pressed`.
    pub async fn pressed(&self) -> Result<bool> {
        Ok(self.prop("ariaPressed").await? == "true")
    }

    /// Whether the control can be worked at all.
    pub async fn enabled(&self) -> Result<bool> {
        Ok(self.inner.is_enabled().await?)
    }

    /// Whether the control says it is out of play, by `aria-disabled`. This is
    /// the question to ask of a custom control: `enabled` reads the form
    /// control's `disabled`, which an element that is not a form control
    /// cannot carry, so it answers `true` for one however dead it is.
    pub async fn disabled(&self) -> Result<bool> {
        Ok(self.prop("ariaDisabled").await? == "true")
    }

    common!();
}
