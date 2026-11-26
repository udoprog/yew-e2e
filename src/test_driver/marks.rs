//! Marks left on the page and on elements, to tell later whether they
//! survived: a client-side navigation from a reload, and an element a render
//! kept from one it replaced.

use anyhow::{Context, Result};

use crate::{IntoBy, TestDriver, TestElement};

/// The `window` property [`TestDriver::mark_page`] sets and
/// [`TestDriver::page_kept`] reads.
const PAGE_MARK: &str = "__e2eKeptPage";

/// The element property [`TestDriver::mark`] sets and
/// [`TestDriver::kept_marked`] reads.
const ELEMENT_MARK: &str = "__e2eMarked";

impl TestDriver {
    /// **Mark the document now showing**, so that [`Self::page_kept`] can
    /// tell later whether it is still the same one. The mark lives on
    /// `window` in the page's own memory, so a client-side navigation keeps
    /// it and anything that loads the document again (a reload, a link the
    /// app does not route itself) loses it.
    pub async fn mark_page(&self) -> Result<()> {
        tracing::trace!("Marking the page");

        self.inner
            .execute(
                "window[arguments[0]] = true;",
                vec![serde_json::json!(PAGE_MARK)],
            )
            .await?;

        Ok(())
    }

    /// **Whether the document marked by [`Self::mark_page`] is still the one
    /// showing**: `true` after a client-side navigation, `false` once the page
    /// has been loaded again, and `false` where it was never marked.
    pub async fn page_kept(&self) -> Result<bool> {
        let kept = self
            .inner
            .execute(
                "return window[arguments[0]] === true;",
                vec![serde_json::json!(PAGE_MARK)],
            )
            .await?;

        kept.json()
            .as_bool()
            .context("whether the page kept its mark")
    }

    /// **Mark every element `by` matches right now**, and say how many that
    /// was, so that [`Self::kept_marked`] can count later how many of the
    /// elements matching then are the very same nodes: a render that keeps an
    /// element keeps its mark, one that replaces it with an equal new one
    /// does not. The mark is a property of the node, not an attribute, so the
    /// page cannot see it in its markup. Marking again adds to what is
    /// already marked.
    pub async fn mark(&self, by: impl IntoBy) -> Result<usize> {
        let by = by.into_by();
        tracing::trace!(?by, "Marking elements");

        let elements = self.find_all(by).await?;

        self.inner
            .execute(
                "for (const el of arguments[0]) el[arguments[1]] = true;",
                vec![elements_json(&elements)?, serde_json::json!(ELEMENT_MARK)],
            )
            .await?;

        Ok(elements.len())
    }

    /// **How many of the elements `by` matches right now carry the mark
    /// [`Self::mark`] left**: those it marked that a render kept. An element
    /// that went away, or was replaced by a new one, is not counted.
    pub async fn kept_marked(&self, by: impl IntoBy) -> Result<usize> {
        let by = by.into_by();
        tracing::trace!(?by, "Counting marked elements kept");

        let elements = self.find_all(by).await?;

        let kept = self
            .inner
            .execute(
                "return arguments[0].filter(el => el[arguments[1]] === true).length;",
                vec![elements_json(&elements)?, serde_json::json!(ELEMENT_MARK)],
            )
            .await?;

        let kept = kept
            .json()
            .as_u64()
            .context("how many marked elements were kept")?;
        Ok(usize::try_from(kept)?)
    }
}

/// Elements as one script argument, which the page reads as an array of them.
fn elements_json(elements: &[TestElement]) -> Result<serde_json::Value> {
    Ok(serde_json::Value::Array(
        elements
            .iter()
            .map(|element| element.inner.to_json())
            .collect::<Result<_, _>>()?,
    ))
}
