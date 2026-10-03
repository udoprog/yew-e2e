//! Giving a field or an element what a hand could not.

use anyhow::Result;

use crate::{TestDriver, TestElement};

impl TestDriver {
    /// **Give a field a value the way a native control would**, then fire
    /// `event` on it, for fields such as `<input type="color">` whose own
    /// picker the driver cannot reach.
    pub async fn set_value(&self, element: &TestElement, value: &str, event: &str) -> Result<()> {
        tracing::trace!(value, event, "Setting a field's value");

        self.inner
            .execute(
                "arguments[0].value = arguments[1]; \
                 arguments[0].dispatchEvent(new Event(arguments[2], {bubbles: true}));",
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(value),
                    serde_json::json!(event),
                ],
            )
            .await?;

        Ok(())
    }

    /// **Replace the text an element already shows**, for content the test
    /// machine cannot be relied on to produce, such as a very long name. Only the existing text node's data changes, so the
    /// page keeps owning the node.
    pub async fn set_text(&self, element: &TestElement, text: &str) -> Result<()> {
        tracing::trace!(text, "Replacing an element's text");

        let script = "const node = arguments[0].firstChild; \
             if (!node || node.nodeType !== Node.TEXT_NODE) throw new Error('no text node'); \
             node.data = arguments[1];";

        self.inner
            .execute(
                script,
                vec![element.inner.to_json()?, serde_json::json!(text)],
            )
            .await?;

        Ok(())
    }
}
