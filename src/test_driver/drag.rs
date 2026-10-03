//! Carrying one element onto another, with the pointer or through the HTML
//! drag handlers.

use anyhow::Result;

use crate::{TestDriver, TestElement};

impl TestDriver {
    /// **Carry one element over another with the pointer, the way a hand
    /// does, and keep hold**: press on it, move off in small steps so the
    /// browser starts its own drag, and come to rest over the target. Unlike
    /// [`Self::drag_over`] nothing is dispatched, so whatever stops the browser
    /// starting a drag, or a target taking one, stops this too. Let go with
    /// [`Self::native_release`].
    pub async fn native_drag_over(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        tracing::trace!("Carrying one element over another with the pointer");

        from.inner.scroll_into_view().await?;

        self.inner
            .action_chain()
            .move_to_element_center(&from.inner)
            .click_and_hold()
            .move_by_offset(4, 0)
            .move_by_offset(8, 2)
            .move_by_offset(16, 4)
            .move_to_element_center(&to.inner)
            .move_by_offset(2, 0)
            .perform()
            .await?;
        self.holding.set(true);

        Ok(())
    }

    /// Let go of what [`Self::native_drag_over`] is holding, where it is.
    pub async fn native_release(&self) -> Result<()> {
        self.inner
            .action_chain()
            .move_by_offset(1, 0)
            .release()
            .perform()
            .await?;
        self.holding.set(false);
        Ok(())
    }

    /// Load real local files into a browser FileList without starting an import.
    pub async fn external_files(&self, paths: &str) -> Result<TestElement> {
        self.inner.execute("const input = document.createElement('input'); input.type = 'file'; input.multiple = true; input.hidden = true; input.dataset.test = 'external-files'; document.body.appendChild(input);", Vec::new()).await?;
        let input = self.find_one_by("[data-test='external-files']").await?;
        input.send_keys(paths).await?;
        Ok(input)
    }

    /// Dispatch the browser file payload onto a drop target; report cancellation.
    pub async fn drag_external_files(
        &self,
        files: &TestElement,
        to: &TestElement,
        kind: &str,
    ) -> Result<bool> {
        let result = self.inner.execute("const data = new DataTransfer(); for (const file of arguments[0].files) data.items.add(file); const event = new DragEvent(arguments[2], {dataTransfer: data, bubbles: true, cancelable: true}); arguments[1].dispatchEvent(event); return event.defaultPrevented;", vec![files.inner.to_json()?, to.inner.to_json()?, kind.into()]).await?;
        Ok(serde_json::from_value(result.json().clone())?)
    }

    /// Carry one element onto another through its HTML drag handlers.
    pub async fn drag_onto(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        tracing::trace!("Carrying one element onto another");

        let script = "const data = new DataTransfer(); \
             const fire = (node, kind) => node.dispatchEvent( \
                 new DragEvent(kind, \
                     { dataTransfer: data, bubbles: true, cancelable: true })); \
             fire(arguments[0], 'dragstart'); \
             fire(arguments[1], 'dragover'); \
             fire(arguments[1], 'drop'); \
             fire(arguments[0], 'dragend');";

        self.inner
            .execute(script, vec![from.inner.to_json()?, to.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Carry one element over another without letting go, so that what the
    /// controller says about the offer can be read.
    pub async fn drag_over(&self, from: &TestElement, to: &TestElement) -> Result<()> {
        tracing::trace!("Carrying one element over another");

        let script = "const data = new DataTransfer(); \
             const fire = (node, kind) => node.dispatchEvent( \
                 new DragEvent(kind, \
                     { dataTransfer: data, bubbles: true, cancelable: true })); \
             fire(arguments[0], 'dragstart'); \
             fire(arguments[1], 'dragover');";

        self.inner
            .execute(script, vec![from.inner.to_json()?, to.inner.to_json()?])
            .await?;

        Ok(())
    }

    /// Carry a drag in progress back off the element it was over, which is the
    /// `dragleave` a browser sends and neither [`Self::drag_over`] nor
    /// [`Self::drag_onto`] does: both begin their own drag, so this is handed
    /// the element the carry is leaving and nothing else.
    pub async fn drag_off(&self, left: &TestElement) -> Result<()> {
        tracing::trace!("Carrying a drag back off an element");

        let script = "const data = new DataTransfer(); \
             arguments[0].dispatchEvent( \
                 new DragEvent('dragleave', \
                     { dataTransfer: data, bubbles: true, cancelable: true }));";

        self.inner
            .execute(script, vec![left.inner.to_json()?])
            .await?;

        Ok(())
    }
}
