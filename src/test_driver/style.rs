//! What the browser settled on for a style, and forcing one.

use anyhow::{Context, Result};

use super::finite;
use crate::{TestDriver, TestElement};

impl TestDriver {
    /// **Force one inline CSS property on an element**, the same lever a
    /// hand-typed `style=""` pulls, for a size no real window is narrow
    /// enough to produce on its own, such as a popover's own row.
    pub async fn set_style(
        &self,
        element: &TestElement,
        property: &str,
        value: &str,
    ) -> Result<()> {
        tracing::trace!(property, value, "Setting an inline style property");

        self.inner
            .execute(
                "arguments[0].style.setProperty(arguments[1], arguments[2]);",
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(property),
                    serde_json::json!(value),
                ],
            )
            .await?;

        Ok(())
    }

    /// **Force one class on or off an element**, for a state the app has no
    /// way to put that element in yet but a stylesheet rule already answers
    /// for, such as a record button that is capturing and unavailable at once.
    pub async fn set_class(&self, element: &TestElement, class: &str, on: bool) -> Result<()> {
        tracing::trace!(class, on, "Forcing a class");

        self.inner
            .execute(
                "arguments[0].classList.toggle(arguments[1], arguments[2]);",
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(class),
                    serde_json::json!(on),
                ],
            )
            .await?;

        Ok(())
    }

    /// **Every box inside an element that has more to show than it has room
    /// for**, one line each: a scroller with something to scroll, or a box
    /// that clips what it holds. Empty when everything in it fits. A box a
    /// pixel or less across is passed over, since that is how a control is
    /// hidden from sight while staying reachable, and it clips by design.
    pub async fn overflowing(&self, element: &TestElement) -> Result<Vec<String>> {
        tracing::trace!("Finding what overflows inside an element");

        let script = "const out = []; \
             for (const el of [arguments[0], ...arguments[0].querySelectorAll('*')]) { \
                 if (el.clientHeight <= 1 || el.clientWidth <= 1) continue; \
                 const cs = getComputedStyle(el); \
                 const y = el.scrollHeight - el.clientHeight; \
                 const x = el.scrollWidth - el.clientWidth; \
                 const shows = v => v === 'auto' || v === 'scroll' || v === 'hidden' || v === 'clip'; \
                 if ((y > 1 && shows(cs.overflowY)) || (x > 1 && shows(cs.overflowX))) { \
                     const test = el.getAttribute('data-test'); \
                     out.push(el.localName + (el.className ? '.' + String(el.className).trim().split(/\\s+/).join('.') : '') \
                         + (test ? `[data-test=${test}]` : '') \
                         + ` ${cs.overflowX}/${cs.overflowY} over by ${x}x${y}`); \
                 } \
             } \
             return out;";

        let found = self
            .inner
            .execute(script, vec![element.inner.to_json()?])
            .await?;

        Ok(found
            .json()
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_owned))
            .collect())
    }

    /// **How many pixels the document is wider than the window**, zero when
    /// it fits: what a phone-sized window scrolls sideways by. [`Self::overflowing`]
    /// cannot see this, since `<html>` is `overflow: visible` and the window,
    /// not a box in the page, is what scrolls.
    pub async fn page_overflow(&self) -> Result<u32> {
        tracing::trace!("Measuring how far the page overflows the window sideways");

        let over = self
            .inner
            .execute(
                "const root = document.documentElement; \
                 return Math.max(0, root.scrollWidth - root.clientWidth);",
                Vec::new(),
            )
            .await?;

        let over = over.json().as_u64().context("how far the page overflows")?;
        Ok(u32::try_from(over)?)
    }

    /// **Every piece of text inside an element set smaller than `floor`
    /// pixels**, one line each, passing over whatever sits inside `marks`: the
    /// numbers of a scale or a badge's count, which are read at a glance rather
    /// than as words. Text nobody can see is passed over too.
    pub async fn small_text(
        &self,
        element: &TestElement,
        floor: f64,
        marks: &str,
    ) -> Result<Vec<String>> {
        tracing::trace!(floor, marks, "Finding text set below a size");

        let floor = finite("a text size floor", floor)?;

        let script = "const out = []; \
             for (const el of [arguments[0], ...arguments[0].querySelectorAll('*')]) { \
                 if (el.closest(arguments[1])) continue; \
                 const words = Array.from(el.childNodes) \
                     .filter(n => n.nodeType === Node.TEXT_NODE) \
                     .map(n => n.textContent.trim()).join(' ').trim(); \
                 if (!words) continue; \
                 const rect = el.getBoundingClientRect(); \
                 if (rect.width <= 1 || rect.height <= 1) continue; \
                 const cs = getComputedStyle(el); \
                 if (cs.visibility !== 'visible') continue; \
                 const size = parseFloat(cs.fontSize); \
                 if (size < arguments[2]) { \
                     const test = el.getAttribute('data-test'); \
                     out.push(el.localName + (el.className ? '.' + String(el.className).trim().split(/\\s+/).join('.') : '') \
                         + (test ? `[data-test=${test}]` : '') \
                         + ` at ${cs.fontSize}: ${JSON.stringify(words.slice(0, 32))}`); \
                 } \
             } \
             return out;";

        let found = self
            .inner
            .execute(
                script,
                vec![element.inner.to_json()?, serde_json::json!(marks), floor],
            )
            .await?;

        Ok(found
            .json()
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_owned))
            .collect())
    }

    /// **What the browser settled on, rather than what a rule says.**
    pub async fn computed(
        &self,
        element: &TestElement,
        pseudo: &str,
        property: &str,
    ) -> Result<String> {
        tracing::trace!(pseudo, property, "Asking what a property computed to");

        let script = "return getComputedStyle(arguments[0], arguments[1] || null) \
             .getPropertyValue(arguments[2]);";

        let value = self
            .inner
            .execute(
                script,
                vec![
                    element.inner.to_json()?,
                    serde_json::json!(pseudo),
                    serde_json::json!(property),
                ],
            )
            .await?;

        Ok(value.json().as_str().unwrap_or_default().trim().to_string())
    }

    /// Whether the page matches `prefers-reduced-motion: reduce`.
    ///
    /// Where `E2E_MOTION` is set this is an error when the browser disagrees
    /// with it, so a run meant to exercise one preference cannot quietly get
    /// the other. Set it (Firefox only) to test the browser's own media
    /// preference rather than a replacement stylesheet.
    pub async fn reduced_motion(&self) -> Result<bool> {
        let value = self
            .inner
            .execute(
                "return matchMedia('(prefers-reduced-motion: reduce)').matches;",
                Vec::new(),
            )
            .await?;
        let reduced = value
            .json()
            .as_bool()
            .context("reduced-motion media query")?;
        if let Ok(requested) = std::env::var("E2E_MOTION") {
            anyhow::ensure!(
                reduced == (requested == "reduce"),
                "browser did not honor E2E_MOTION={requested}"
            );
        }
        Ok(reduced)
    }

    /// Pause an actual CSS animation at a deterministic phase before reading its
    /// computed presentation. Missing animations are failures, not idle samples.
    pub async fn animation_time(
        &self,
        element: &TestElement,
        name: &str,
        millis: f64,
    ) -> Result<()> {
        self.inner.execute(
            "const animation = arguments[0].getAnimations().find(a => a.animationName === arguments[1]); \
             if (!animation) throw new Error('Missing animation: ' + arguments[1]); \
             animation.pause(); animation.currentTime = arguments[2];",
            vec![element.inner.to_json()?, serde_json::json!(name), serde_json::json!(millis)],
        ).await?;
        Ok(())
    }

    /// Read a computed color's alpha through the browser's color parser so both
    /// legacy rgba() and CSS Color 4 serialization describe the same measurement.
    pub async fn color_alpha(&self, element: &TestElement, property: &str) -> Result<f64> {
        let value = self.inner.execute(
            "const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1; \
             const context = canvas.getContext('2d'); context.fillStyle = 'transparent'; \
             context.fillStyle = getComputedStyle(arguments[0]).getPropertyValue(arguments[1]); \
             context.fillRect(0, 0, 1, 1); return context.getImageData(0, 0, 1, 1).data[3] / 255;",
            vec![element.inner.to_json()?, serde_json::json!(property)],
        ).await?;
        value.json().as_f64().context("computed color alpha")
    }

    /// **Which way a computed colour moves what is painted under it.** The
    /// property is filled over mid grey by the browser itself and the result
    /// compared with it, so a band's direction is measured rather than read off
    /// a serialization: negative darkens, positive lightens, zero paints
    /// nothing. `color-mix()` and a legacy `rgba()` are the same measurement.
    pub async fn color_shift(&self, element: &TestElement, property: &str) -> Result<f64> {
        let [red, green, blue] = self.painted(element, property).await?;
        Ok((red + green + blue) / 3.0 - 128.0)
    }

    /// **What a computed colour actually paints**, as red, green and blue.
    ///
    /// The browser fills the property over mid grey and the pixel is read back,
    /// so a `color-mix()`, a legacy `rgba()` and a translucent wash all answer
    /// as the colour that is seen rather than as whatever the property happens
    /// to serialize to.
    pub async fn painted(&self, element: &TestElement, property: &str) -> Result<[f64; 3]> {
        let value = self.inner.execute(
            "const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1; \
             const context = canvas.getContext('2d'); \
             context.fillStyle = '#808080'; context.fillRect(0, 0, 1, 1); \
             context.fillStyle = 'transparent'; \
             context.fillStyle = getComputedStyle(arguments[0]).getPropertyValue(arguments[1]); \
             context.fillRect(0, 0, 1, 1); \
             const painted = context.getImageData(0, 0, 1, 1).data; \
             return [painted[0], painted[1], painted[2]];",
            vec![element.inner.to_json()?, serde_json::json!(property)],
        ).await?;

        let read = value.json();
        let read = read.as_array().context("what a computed colour paints")?;

        let mut rgb = [0.0; 3];

        for (channel, value) in rgb.iter_mut().zip(read) {
            *channel = value.as_f64().context("a painted channel")?;
        }

        Ok(rgb)
    }
}
