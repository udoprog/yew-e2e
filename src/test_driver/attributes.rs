//! Reading many matches in one pass inside the page.

use anyhow::{Context, Result};

use crate::TestDriver;

impl TestDriver {
    /// **Every matching rendered text in one browser observation.**
    ///
    /// A live viewer can replace its rows while WebDriver walks a list of
    /// element handles. Read the list inside the document when the
    /// observation itself is the assertion, so it is one snapshot rather
    /// than a handle per row.
    pub async fn rendered_texts(&self, selector: &str) -> Result<Vec<String>> {
        let read = self
            .inner
            .execute(
                "return Array.from(document.querySelectorAll(arguments[0])) \
                 .map(n => n.innerText)",
                vec![serde_json::json!(selector)],
            )
            .await?;
        let rows = read.json().as_array().context("rendered text array")?;

        rows.iter()
            .map(|row| row.as_str().context("rendered text").map(str::to_owned))
            .collect()
    }

    /// **Every match's attribute, read in one pass.** Reading them one handle
    /// at a time is how a test goes stale against a window that redraws on the
    /// animation frame: the elements it is holding are replaced between two reads
    /// and the second one throws.
    pub async fn find_all_attrs(&self, selector: &str, attr: &str) -> Result<Vec<String>> {
        let read = self
            .inner
            .execute(
                "return Array.from(document.querySelectorAll(arguments[0])) \
                 .map(n => n.getAttribute(arguments[1]) ?? '')",
                vec![serde_json::json!(selector), serde_json::json!(attr)],
            )
            .await?;

        Ok(read
            .json()
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|row| row.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default())
    }

    /// **Where every match sits down the page, with the attribute that names
    /// it, in one pass.** A band is a top and a height, which is all two things
    /// drawn in different columns have to agree about.
    ///
    /// Measuring a hundred elements one handle at a time is a round trip each
    /// for the attribute and another for the box, which is a test that runs out
    /// of time rather than one that fails. A node the browser is not drawing has
    /// no box and is left out, so a count is also an answer to what is shown.
    pub async fn bands(&self, selector: &str, attr: &str) -> Result<Vec<(String, f64, f64)>> {
        let script = "return Array.from(document.querySelectorAll(arguments[0])) \
             .filter(n => n.getClientRects().length > 0) \
             .map(n => [n.getAttribute(arguments[1]) ?? '', \
             n.getBoundingClientRect().top, n.getBoundingClientRect().height])";

        let read = self
            .inner
            .execute(
                script,
                vec![serde_json::json!(selector), serde_json::json!(attr)],
            )
            .await?;
        let read = read.json();

        let rows = read.as_array().context("a list of bands")?;
        let mut bands = Vec::with_capacity(rows.len());

        for row in rows {
            let row = row.as_array().context("a band")?;

            bands.push((
                row.first()
                    .and_then(|name| name.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                row.get(1).and_then(|top| top.as_f64()).context("a top")?,
                row.get(2)
                    .and_then(|height| height.as_f64())
                    .context("a height")?,
            ));
        }

        Ok(bands)
    }
}
