//! Screenshots, of the page or of the frame it is shown in, and downloads.

use anyhow::Result;
use thirtyfour::By;

use super::navigation::FRAME_MARK;
use crate::TestDriver;
use crate::driver::with_cleanup;

impl TestDriver {
    /// A screenshot of the whole page as the PNG the browser hands back, for a
    /// test that reads a frame rather than writing one out. Taken whether or
    /// not `E2E_SHOTS` is set, since an assertion that measures a frame has to
    /// have one on every run rather than only when somebody is looking.
    ///
    /// Inside the frame [`Self::in_frame`] entered, it is of that frame alone,
    /// and the driver is left in the frame.
    pub async fn screenshot_png(&self) -> Result<Vec<u8>> {
        if !self.in_marked_frame().await? {
            tracing::trace!("Taking a screenshot");
            return Ok(self.inner.screenshot_as_png().await?);
        }

        tracing::trace!("Taking a screenshot of the frame");

        self.inner.enter_parent_frame().await?;

        let frame = self
            .inner
            .find(By::Css(format!("iframe[{FRAME_MARK}]")))
            .await;

        let frame = match frame {
            Ok(frame) => frame,
            Err(error) => {
                let back = self.inner.enter_frame(0).await.map_err(Into::into);
                return Err(with_cleanup(
                    anyhow::Error::from(error).context("finding the frame to screenshot"),
                    back,
                ));
            }
        };

        let png = frame.screenshot_as_png().await;
        let back = frame.enter_frame().await;

        match (png, back) {
            (Ok(png), Ok(())) => Ok(png),
            (Ok(_), Err(back)) => {
                Err(anyhow::Error::from(back).context("entering the frame again"))
            }
            (Err(error), back) => Err(with_cleanup(error.into(), back.map_err(Into::into))),
        }
    }

    /// Write a screenshot of the whole page to `path`.
    pub async fn screenshot(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        std::fs::write(path, self.screenshot_png().await?)?;
        Ok(())
    }

    /// The same, into `$E2E_SHOTS/<name>.png`, and nothing at all when that is
    /// unset. Left in a test rather than added and removed, since it costs nothing
    /// on an ordinary run.
    ///
    /// **`E2E_SHOTS` is a directory, and it reads like a flag.** A relative one
    /// is taken under `target/e2e-shots`, where everything else the suite leaves
    /// behind goes — so `E2E_SHOTS=1`, which is what a hand types when it means
    /// *yes*, writes to `target/e2e-shots/1` rather than making a directory
    /// called `1` in whatever the test process's working directory happens to
    /// be, which under cargo is the package, in the checkout.
    ///
    /// ```text
    /// E2E_SHOTS=1 cargo test -p app-e2e -- cart::
    /// E2E_SHOTS=/tmp/shots cargo test -p app-e2e -- cart::
    /// ```
    pub async fn snapshot(&self, name: &str) -> Result<()> {
        let Some(dir) = std::env::var_os("E2E_SHOTS") else {
            return Ok(());
        };

        let dir = std::path::PathBuf::from(dir);

        let dir = match dir.is_absolute() {
            true => dir,
            false => crate::dist::target_dir()?.join("e2e-shots").join(dir),
        };

        std::fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{name}.png"));
        tracing::info!(path = %path.display(), "Writing snapshot");
        self.screenshot(&path).await
    }
}
