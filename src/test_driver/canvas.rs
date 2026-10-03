//! What a canvas drew, and what is drawn at a point.

use anyhow::{Context, Result};

use crate::{TestDriver, TestElement};

/// A canvas's own reported size -- see [`TestDriver::canvas_size`].
#[derive(Debug, Clone, Copy)]
pub struct CanvasSize {
    /// The backing buffer's width, in device pixels.
    pub device_width: f64,
    /// The backing buffer's height, in device pixels.
    pub device_height: f64,
    /// The element's own layout box width, in CSS pixels.
    pub css_width: f64,
    /// The element's own layout box height, in CSS pixels.
    pub css_height: f64,
    /// What the browser itself reports for `window.devicePixelRatio`.
    pub device_pixel_ratio: f64,
}

impl TestDriver {
    /// **A canvas's own drawn envelope**, read from its actual pixel buffer
    /// rather than any DOM or SVG geometry: a `<canvas>` carries no
    /// `points`/`viewBox` attribute a test could read instead. Samples up to `columns` evenly
    /// spaced points across the canvas's backing-buffer width; each entry is
    /// `(x, top, bottom)`, all fractions (0.0..=1.0) of the canvas's own box
    /// -- `x` across its width, `top`/`bottom` the first and last drawn
    /// (non-transparent) pixel row down its height. A sampled column with
    /// nothing drawn reads `(x, -1.0, -1.0)`.
    pub async fn canvas_columns(
        &self,
        canvas: &TestElement,
        columns: usize,
    ) -> Result<Vec<(f64, f64, f64)>> {
        let script = "const canvas = arguments[0]; \
             const columns = Math.max(1, arguments[1]); \
             const w = canvas.width, h = canvas.height; \
             if (w === 0 || h === 0) return []; \
             const ctx = canvas.getContext('2d'); \
             const data = ctx.getImageData(0, 0, w, h).data; \
             const step = Math.max(1, Math.floor(w / columns)); \
             const out = []; \
             for (let x = 0; x < w; x += step) { \
                 let top = -1, bottom = -1; \
                 for (let y = 0; y < h; y++) { \
                     if (data[(y * w + x) * 4 + 3] > 0) { \
                         if (top === -1) top = y; \
                         bottom = y; \
                     } \
                 } \
                 out.push([x / w, top === -1 ? -1 : top / h, bottom === -1 ? -1 : bottom / h]); \
             } \
             return out;";

        let value = self
            .inner
            .execute(
                script,
                vec![canvas.inner.to_json()?, serde_json::json!(columns)],
            )
            .await?;

        let read = value.json();
        let read = read.as_array().context("canvas column scan")?;
        let mut out = Vec::with_capacity(read.len());

        for row in read {
            let row = row.as_array().context("a canvas column")?;
            let field = |at: usize, name: &str| {
                row.get(at)
                    .and_then(serde_json::Value::as_f64)
                    .with_context(|| format!("a canvas column's {name}"))
            };
            out.push((field(0, "x")?, field(1, "top")?, field(2, "bottom")?));
        }

        Ok(out)
    }

    /// A canvas's backing-buffer size in device pixels alongside its
    /// own CSS box and the browser's reported device pixel ratio, so a caller
    /// can check the three agree -- that the buffer is sized for the actual
    /// visible box and pixel density, not a fixed or stale guess.
    pub async fn canvas_size(&self, canvas: &TestElement) -> Result<CanvasSize> {
        let script = "const c = arguments[0]; const r = c.getBoundingClientRect(); \
             return [c.width, c.height, r.width, r.height, window.devicePixelRatio];";

        let value = self
            .inner
            .execute(script, vec![canvas.inner.to_json()?])
            .await?;
        let read = value.json();
        let read = read.as_array().context("canvas size read")?;
        let field = |at: usize, name: &str| {
            read.get(at)
                .and_then(serde_json::Value::as_f64)
                .with_context(|| format!("canvas size's {name}"))
        };

        Ok(CanvasSize {
            device_width: field(0, "device_width")?,
            device_height: field(1, "device_height")?,
            css_width: field(2, "css_width")?,
            css_height: field(3, "css_height")?,
            device_pixel_ratio: field(4, "device_pixel_ratio")?,
        })
    }

    /// Whether what is drawn at one point of the page belongs to `selector` and
    /// nothing is standing over it. A frame says what is drawn somewhere but
    /// not what drew it, so a test measuring a window's own content has to be
    /// able to tell that content from whatever floats in front of it.
    ///
    /// Two questions, because one does not do. Hit testing says which element
    /// is uppermost at the point, which settles the ordinary case. But a menu,
    /// a drag ghost or a help bubble is drawn through a portal into the
    /// document body, over the top of a window and often wider than it, and a
    /// bubble takes no pointer events at all -- hit testing looks straight
    /// through it and answers with the window underneath. So anything portaled
    /// beside the application is asked separately whether it covers the point.
    ///
    /// The point is in the page's own pixels, not the frame's.
    pub async fn drawn_inside(&self, selector: &str, x: f64, y: f64) -> Result<bool> {
        tracing::trace!(selector, x, y, "Asking what is drawn at a point");

        let script = "const [selector, x, y] = arguments; \
             const host = document.querySelector(selector); \
             const found = document.elementFromPoint(x, y); \
             if (!host || !found || !host.contains(found)) return false; \
             for (const over of document.body.children) { \
                 if (over.contains(host) || host.contains(over)) continue; \
                 const box = over.getBoundingClientRect(); \
                 if (x >= box.left && x <= box.right && y >= box.top && y <= box.bottom) \
                     return false; \
             } \
             return true;";

        let value = self
            .inner
            .execute(
                script,
                vec![
                    serde_json::json!(selector),
                    serde_json::json!(x),
                    serde_json::json!(y),
                ],
            )
            .await?;

        Ok(value.json().as_bool().unwrap_or_default())
    }
}
