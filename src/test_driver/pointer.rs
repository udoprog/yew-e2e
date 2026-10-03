//! Pressing, holding and letting go with the pointer.

use anyhow::{Result, anyhow};
use tokio::time;

use crate::{IntoBy, POLL_INTERVAL, TestDriver, TestElement};

impl TestDriver {
    /// Carry one element onto another and let go.
    pub async fn pointer_drag_onto(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        from.inner.scroll_into_view().await?;
        self.inner
            .action_chain()
            .move_to_element_center(&from.inner)
            .click_and_hold()
            .move_by_offset(8, 0)
            .release_on_element(&to.inner)
            .perform()
            .await?;
        Ok(())
    }

    /// Let go of every key and button the actions above hold, the WebDriver
    /// way, which also puts down a drag an engine would otherwise keep
    /// carrying into the next test.
    ///
    /// A drag begun here is let go where the pointer is, as the specification
    /// asks; chromedriver's own release would let it go where it was pressed.
    pub async fn release_all_input(&self) -> Result<()> {
        if self.holding.take() {
            self.inner.action_chain().release().perform().await?;
        }

        self.inner.action_chain().reset_actions().await?;
        Ok(())
    }

    /// Press the middle button over an element.
    pub async fn middle_click(&self, element: &TestElement) -> Result<()> {
        tracing::trace!("Middle-clicking an element");

        let script = "for (const type of ['mousedown', 'mouseup', 'auxclick']) {\
             arguments[0].dispatchEvent(new MouseEvent(type, \
             { button: 1, buttons: 4, bubbles: true, cancelable: true })); \
         }";

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Pan with a native middle-button drag, releasing the button at the end.
    pub async fn middle_drag_by(&self, element: &TestElement, dx: i64) -> Result<()> {
        use thirtyfour::common::command::Command;

        let actions = serde_json::json!([{
            "type": "pointer", "id": "pointer", "parameters": {"pointerType": "mouse"},
            "actions": [
                {"type": "pointerMove", "duration": 0, "origin": element.inner.to_json()?, "x": 0, "y": 0},
                {"type": "pointerDown", "button": 1},
                {"type": "pointerMove", "duration": 150, "origin": "pointer", "x": dx, "y": 0},
                {"type": "pointerUp", "button": 1}
            ]
        }]);
        self.inner
            .cmd(Command::PerformActions(actions.into()))
            .await?;
        Ok(())
    }

    /// Carry a held drag somewhere else without letting go of it.
    pub async fn move_held(&self, dx: i64, dy: i64) -> Result<()> {
        tracing::trace!(dx, dy, "Carrying a held drag");

        self.inner
            .action_chain()
            .move_by_offset(dx, dy)
            .perform()
            .await?;

        Ok(())
    }

    /// Let go of whatever [`TestElement::drag_by`] is holding.
    pub async fn drop_held(&self) -> Result<()> {
        tracing::trace!("Releasing a held drag");
        self.inner.action_chain().release().perform().await?;
        self.holding.set(false);
        Ok(())
    }

    /// Cancel a captured pointer without releasing the test's held button, so
    /// a later mouse-up can prove that cancelled work is not committed.
    pub async fn cancel_pointer(&self, element: &TestElement) -> Result<()> {
        self.inner
            .execute(
                "arguments[0].dispatchEvent(new PointerEvent('pointercancel', { bubbles: true }));",
                vec![element.inner.to_json()?],
            )
            .await?;
        Ok(())
    }

    /// The same for a drag that was begun with `alt` held, which has to give
    /// the key back as well as the button.
    pub async fn drop_held_alt(&self) -> Result<()> {
        tracing::trace!("Releasing a held drag and the alt key with it");

        self.inner
            .action_chain()
            .release()
            .key_up(thirtyfour::Key::Alt)
            .perform()
            .await?;
        self.holding.set(false);

        Ok(())
    }

    /// Press the `index`th element matching `by`, re-finding it if the page
    /// moves underneath.
    ///
    /// Giving up says why the last press did not land, such as another
    /// element covering this one, under the timeout.
    pub async fn press_nth(&self, by: impl IntoBy, index: usize) -> Result<()> {
        let by = by.into_by();

        tracing::trace!(?by, index, "Pressing element");

        let deadline = time::Instant::now() + self.wait_timeout;
        let mut refused = None;

        loop {
            let found = self.find_all(by.clone()).await?;

            if let Some(item) = found.get(index) {
                match item.click().await {
                    Ok(()) => return Ok(()),
                    Err(error) => refused = Some(error),
                }
            }

            if time::Instant::now() >= deadline {
                let timeout = format!(
                    "Timed out after {:?} pressing {by} at {index}",
                    self.wait_timeout
                );

                return Err(match refused {
                    Some(error) => error.context(timeout),
                    None => anyhow!(timeout),
                });
            }

            time::sleep(POLL_INTERVAL).await;
        }
    }

    /// The same as [`TestDriver::press_nth`], right-handed. A context menu that
    /// belongs to a thing is opened on the thing itself, so there is no button
    /// to press and this is how a test reaches one.
    pub async fn context_press_nth(&self, by: impl IntoBy, index: usize) -> Result<()> {
        let by = by.into_by();

        tracing::trace!(?by, index, "Right-clicking element");

        let deadline = time::Instant::now() + self.wait_timeout;
        let mut refused = None;

        loop {
            let found = self.find_all(by.clone()).await?;

            if let Some(item) = found.get(index) {
                match item.context_click().await {
                    Ok(()) => return Ok(()),
                    Err(error) => refused = Some(error),
                }
            }

            if time::Instant::now() >= deadline {
                let timeout = format!(
                    "Timed out after {:?} right-clicking {by} at {index}",
                    self.wait_timeout
                );

                return Err(match refused {
                    Some(error) => error.context(timeout),
                    None => anyhow!(timeout),
                });
            }

            time::sleep(POLL_INTERVAL).await;
        }
    }
}
