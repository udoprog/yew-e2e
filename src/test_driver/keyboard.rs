//! Keys, pressed through WebDriver or dispatched on an element, and focus.

use anyhow::Result;
use thirtyfour::prelude::*;

use crate::TestDriver;

impl TestDriver {
    /// Press a key with nothing in particular focused, the way a hand does when
    /// it has just opened something with the pointer.
    pub async fn press_key(&self, key: Key) -> Result<()> {
        tracing::trace!(?key, "Pressing a key");
        self.inner.action_chain().send_keys(key).perform().await?;
        Ok(())
    }

    /// Hold a real WebDriver key across observations, rather than sending a
    /// down/up pair too quickly for an animation frame to show it.
    pub async fn hold_key(&self, key: char) -> Result<()> {
        self.inner.action_chain().key_down(key).perform().await?;
        Ok(())
    }

    /// Let go of a key held with [`Self::hold_key`].
    pub async fn release_key(&self, key: char) -> Result<()> {
        self.inner.action_chain().key_up(key).perform().await?;
        Ok(())
    }

    /// Dispatch a named key on a focused element, including keys WebDriver cannot encode.
    pub async fn press_key_on(&self, selector: &str, key: &str) -> Result<()> {
        let script = "const at = document.querySelector(arguments[0]); \
             if (!at) throw new Error('No element for keyboard press'); \
             at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             { key: arguments[1], bubbles: true, cancelable: true }));";
        self.inner
            .execute(
                script,
                vec![serde_json::json!(selector), serde_json::json!(key)],
            )
            .await?;
        Ok(())
    }

    /// **Press a key on one element with `control` held**, which is how undo,
    /// redo and select-all are reached.
    ///
    /// Dispatched on the element so the press lands where the test says. The
    /// event carries the `code` a real letter key does, since a listener on
    /// the document may read the code and a press without one would never
    /// reach it.
    pub async fn press_key_with_ctrl(&self, selector: &str, key: &str) -> Result<()> {
        tracing::trace!(selector, key, "Pressing a key with control held");

        let code = match key.chars().collect::<Vec<_>>().as_slice() {
            [c] if c.is_ascii_alphabetic() => format!("Key{}", c.to_ascii_uppercase()),
            _ => key.to_owned(),
        };

        let script = "const at = document.querySelector(arguments[0]); \
             if (at) { at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             { key: arguments[1], code: arguments[2], ctrlKey: true, bubbles: true, cancelable: true })); }";

        self.inner
            .execute(
                script,
                vec![
                    serde_json::json!(selector),
                    serde_json::json!(key),
                    serde_json::json!(code),
                ],
            )
            .await?;
        Ok(())
    }

    /// **Press a key on one element with shift held**, dispatched for the
    /// same reason [`Self::press_key_with_ctrl`] is.
    pub async fn press_key_with_shift(&self, selector: &str, key: &str) -> Result<()> {
        tracing::trace!(selector, key, "Pressing a key with shift held");

        let script = "const at = document.querySelector(arguments[0]); \
             if (at) { at.focus(); at.dispatchEvent(new KeyboardEvent('keydown', \
             { key: arguments[1], shiftKey: true, bubbles: true, cancelable: true })); }";

        self.inner
            .execute(
                script,
                vec![serde_json::json!(selector), serde_json::json!(key)],
            )
            .await?;
        Ok(())
    }

    /// Take keyboard focus off whatever has it, so the next key lands on the
    /// page rather than on a control.
    pub async fn blur(&self) -> Result<()> {
        tracing::trace!("Blurring the focused element");

        self.inner
            .execute(
                "document.activeElement && document.activeElement.blur();",
                Vec::new(),
            )
            .await?;

        Ok(())
    }

    /// One attribute of whatever element has keyboard focus, or `None` when
    /// it has no such attribute. Where nothing is focused, that is the
    /// document's `<body>`.
    ///
    /// Answers once, at the time of asking; it does not wait.
    pub async fn focused_attr(&self, name: &str) -> Result<Option<String>> {
        let found = self
            .inner
            .execute(
                "return document.activeElement \
                 && document.activeElement.getAttribute(arguments[0])",
                vec![serde_json::json!(name)],
            )
            .await?;

        Ok(found.json().as_str().map(str::to_owned))
    }
}
