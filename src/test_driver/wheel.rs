//! Turning the wheel over an element.

use anyhow::Result;

use super::finite;
use crate::{TestDriver, TestElement};

impl TestDriver {
    /// **Turn the wheel and report whether anything consumed it.**
    ///
    /// Dispatches the same event as [`Self::wheel`], and returns `true` when a
    /// listener called `preventDefault()` on it.
    pub async fn wheel_consumed(&self, element: &TestElement, delta: f64) -> Result<bool> {
        tracing::trace!(delta, "Turning the wheel and asking who took it");

        let delta = finite("a wheel delta", delta)?;

        let taken = self
            .inner
            .execute(
                "return !arguments[0].dispatchEvent(new WheelEvent('wheel', \
                 { deltaY: arguments[1], bubbles: true, cancelable: true }))",
                vec![element.inner.to_json()?, delta],
            )
            .await?;

        Ok(taken.json().as_bool().unwrap_or(false))
    }

    /// Turn the wheel over an element: a cancelable, bubbling `wheel` event
    /// with `deltaY` set to `delta` (positive scrolls down), dispatched on it.
    ///
    /// The event is synthesized in the page rather than sent through
    /// WebDriver, so it reaches the element's listeners but the browser does
    /// not scroll anything by itself. Returns once the event is dispatched;
    /// see [`Self::wheel_consumed`] to learn whether anything handled it.
    pub async fn wheel(&self, element: &TestElement, delta: f64) -> Result<()> {
        tracing::trace!(delta, "Turning the wheel over an element");

        let delta = finite("a wheel delta", delta)?;

        self.inner
            .execute(
                "arguments[0].dispatchEvent(new WheelEvent('wheel', \
                 { deltaY: arguments[1], bubbles: true, cancelable: true }))",
                vec![element.inner.to_json()?, delta],
            )
            .await?;

        Ok(())
    }

    /// **Turn the wheel with `alt` held**, and `shift` too where asked.
    ///
    /// The pointer is put at the middle of the element rather than left at the
    /// origin a fresh `WheelEvent` carries: this zoom holds still whatever is
    /// under the pointer, so a notch at `clientY` zero anchors on the top edge
    /// and walks what is shown straight out of the window.
    pub async fn wheel_alt(&self, element: &TestElement, delta: f64, shift: bool) -> Result<()> {
        tracing::trace!(delta, shift, "Turning the wheel with alt held");

        let delta = finite("a wheel delta", delta)?;

        let script = "const box = arguments[0].getBoundingClientRect(); \
             arguments[0].dispatchEvent(new WheelEvent('wheel', \
             { deltaY: arguments[1], altKey: true, shiftKey: arguments[2], \
             clientX: box.left + box.width / 2, \
             clientY: box.top + box.height / 2, \
             bubbles: true, cancelable: true }))";

        self.inner
            .execute(
                script,
                vec![element.inner.to_json()?, delta, serde_json::json!(shift)],
            )
            .await?;

        Ok(())
    }

    /// **Turn the wheel over an element and say whether the page took it.**
    ///
    /// Both axes and `ctrl` are given, because that is what tells one turn from
    /// another where an element answers for more than one of them, and the
    /// pointer is put in the middle of the element so a zoom that holds still
    /// whatever is under it has something to hold.
    ///
    /// What comes back is whether the default was prevented, which is the only
    /// thing that separates a gesture the window answered from one it answered
    /// *and* let fall through to scroll the box it landed in.
    pub async fn wheel_over(
        &self,
        element: &TestElement,
        across: f64,
        down: f64,
        ctrl: bool,
    ) -> Result<bool> {
        tracing::trace!(across, down, ctrl, "Turning the wheel over both axes");

        let across = finite("a sideways wheel delta", across)?;
        let down = finite("a wheel delta", down)?;

        let script = "const box = arguments[0].getBoundingClientRect(); \
             return !arguments[0].dispatchEvent(new WheelEvent('wheel', \
             { deltaX: arguments[1], deltaY: arguments[2], ctrlKey: arguments[3], \
             clientX: box.left + box.width / 2, \
             clientY: box.top + box.height / 2, \
             bubbles: true, cancelable: true }))";

        let taken = self
            .inner
            .execute(
                script,
                vec![
                    element.inner.to_json()?,
                    across,
                    down,
                    serde_json::json!(ctrl),
                ],
            )
            .await?;

        Ok(taken.json().as_bool().unwrap_or(false))
    }

    /// **Turn the wheel sideways**, reading as `deltaX`: the gesture a trackpad
    /// makes to pan, and not the one `wheel` makes.
    pub async fn wheel_across(&self, element: &TestElement, delta: f64) -> Result<()> {
        tracing::trace!(delta, "Turning the wheel sideways over an element");

        let delta = finite("a sideways wheel delta", delta)?;

        self.inner
            .execute(
                "arguments[0].dispatchEvent(new WheelEvent('wheel', \
                 { deltaX: arguments[1], bubbles: true, cancelable: true }))",
                vec![element.inner.to_json()?, delta],
            )
            .await?;

        Ok(())
    }
}
