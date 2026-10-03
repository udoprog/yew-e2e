//! `localStorage`, read, seeded and blocked.

use anyhow::Result;

use crate::TestDriver;

impl TestDriver {
    /// Read a browser-persisted setting without changing it.
    pub async fn local_storage(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .inner
            .execute(
                "return localStorage.getItem(arguments[0]);",
                vec![serde_json::json!(key)],
            )
            .await?;
        Ok(value.json().as_str().map(str::to_owned))
    }

    /// Seed a persisted setting before mounting the component that reads it.
    pub async fn set_local_storage(&self, key: &str, value: &str) -> Result<()> {
        self.inner
            .execute(
                "localStorage.setItem(arguments[0], arguments[1]);",
                vec![serde_json::json!(key), serde_json::json!(value)],
            )
            .await?;
        Ok(())
    }

    /// Deny access for the current document, as when browser policy blocks storage.
    pub async fn block_local_storage(&self) -> Result<()> {
        self.inner.execute(
            "const script = document.createElement('script'); \
             script.textContent = `Object.defineProperty(window, 'localStorage', {configurable: true, get() { throw new DOMException('Storage blocked by test', 'SecurityError'); }});`; \
             document.documentElement.appendChild(script); script.remove();",
            vec![],
        ).await?;
        Ok(())
    }
}
