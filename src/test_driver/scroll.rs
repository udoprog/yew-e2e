//! Moving what a box shows.

use anyhow::Result;

use crate::{TestDriver, TestElement};

impl TestDriver {
    /// Set a container's vertical scroll, letting the browser emit its scroll event.
    pub async fn scroll_top(&self, element: &TestElement, top: i32) -> Result<()> {
        self.inner
            .execute(
                "arguments[0].scrollTop = arguments[1];",
                vec![element.inner.to_json()?, serde_json::json!(top)],
            )
            .await?;
        Ok(())
    }

    /// Pin whatever scrolls this element sideways to the far end of its travel.
    pub async fn scroll_to_far_end(&self, element: &TestElement) -> Result<()> {
        tracing::trace!("Scrolling an element's box to the end of its travel");

        let script = "let node = arguments[0].parentElement; \
             while (node) { \
                 const how = getComputedStyle(node).overflowX; \
                 if ((how === 'auto' || how === 'scroll') \
                     && node.scrollWidth > node.clientWidth) { \
                     break; \
                 } \
                 node = node.parentElement; \
             } \
             if (node) { node.scrollLeft = node.scrollWidth; }";

        self.inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(())
    }
}
