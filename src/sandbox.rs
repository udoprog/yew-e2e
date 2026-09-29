//! **The directory a run's application is confined to**, and the check that it
//! left it as it found it.
//!
//! The root is made fresh under the build's own target directory, per process,
//! so two runs in one worktree never share it. What confining means is the
//! fixture's to say; see [`crate::Fixture::enter_sandbox`].

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// A run's sandbox, and what was in it before any fixture started.
pub(crate) struct Sandbox {
    root: PathBuf,
    before: BTreeSet<PathBuf>,
}

impl Sandbox {
    /// Make the root, hand it to `confine`, and write down what is there.
    pub(crate) fn enter(confine: impl FnOnce(&Path) -> Result<()>) -> Result<Self> {
        let root = crate::dist::target_dir()?
            .join("e2e-sandbox")
            .join(std::process::id().to_string());

        if root.exists() {
            fs::remove_dir_all(&root)
                .with_context(|| format!("clearing a stale sandbox at {}", root.display()))?;
        }

        fs::create_dir_all(&root)
            .with_context(|| format!("making the sandbox at {}", root.display()))?;

        let root = root
            .canonicalize()
            .with_context(|| format!("resolving the sandbox at {}", root.display()))?;

        confine(&root)?;

        let before = entries(&root)?;

        tracing::info!("The application is confined to {}", root.display());

        Ok(Self { root, before })
    }

    /// **Fail loudly if anything was written into the sandbox**, and remove it
    /// if nothing was. A sandbox with something new in it is kept, so what
    /// wrote there can be read off what it left.
    pub(crate) fn finish(self) -> Result<()> {
        let after = entries(&self.root)?;
        let new = after.difference(&self.before).collect::<Vec<_>>();

        if !new.is_empty() {
            let listed = new
                .iter()
                .map(|path| format!("    {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n");

            bail!(
                "the application wrote into its sandbox; kept at {} for reading:\n{listed}",
                self.root.display()
            );
        }

        fs::remove_dir_all(&self.root)
            .with_context(|| format!("removing the sandbox at {}", self.root.display()))
    }
}

/// Every path under `root`, directories included.
fn entries(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root.to_owned()];

    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();

            if path.is_dir() {
                pending.push(path.clone());
            }

            found.insert(path);
        }
    }

    Ok(found)
}
