//! The frontend the suite serves, built by the suite rather than by hand.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

use crate::Frontend;

static DIST: OnceLock<PathBuf> = OnceLock::new();

/// Where this run's frontend is, once [`Frontend`] has been built.
pub fn dist() -> Result<&'static Path> {
    let dist = DIST
        .get()
        .context("the frontend has not been built; see `Fixture::config`")?;

    Ok(dist.as_path())
}

/// Build the frontend, once, for the whole run.
pub(crate) async fn build(frontend: &Frontend) -> Result<()> {
    if DIST.get().is_some() {
        return Ok(());
    }

    let Frontend::Trunk(dir) = frontend else {
        return Ok(());
    };

    let dist = resolve(dir).await?;
    let _ = DIST.set(dist);
    Ok(())
}

/// The build's own target directory, which is where everything the suite leaves
/// behind goes.
pub fn target_dir() -> Result<PathBuf> {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        return Ok(PathBuf::from(target));
    }

    Ok(workspace_root()?.join("target"))
}

/// The workspace root: the nearest directory above the package under test
/// whose `Cargo.toml` declares a `[workspace]`, or the package itself where
/// none does.
///
/// Cargo says where the package is while it runs a test. A binary run outside
/// cargo has no such thing to go by, and the directory it was started in is
/// searched upwards instead.
pub fn workspace_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let dir = PathBuf::from(dir);
        return Ok(search(&dir).unwrap_or(dir));
    }

    let from = std::env::current_dir().context("locating the current directory")?;

    search(&from).or_else(|| package(&from)).with_context(|| {
        format!(
            "locating the workspace root from {}: run under cargo, or from inside the workspace",
            from.display()
        )
    })
}

/// The nearest enclosing directory that is a workspace root, starting at
/// `from` and walking upwards.
fn search(from: &Path) -> Option<PathBuf> {
    from.ancestors()
        .find(|dir| is_root(dir))
        .map(Path::to_owned)
}

/// The nearest enclosing package, for a package that is its own workspace.
fn package(from: &Path) -> Option<PathBuf> {
    from.ancestors()
        .find(|dir| dir.join("Cargo.toml").is_file())
        .map(Path::to_owned)
}

/// Whether this directory's manifest declares a workspace.
fn is_root(dir: &Path) -> bool {
    let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };

    manifest.lines().any(|line| line.trim() == "[workspace]")
}

async fn resolve(dir: &Path) -> Result<PathBuf> {
    if let Some(given) = std::env::var_os("E2E_DIST") {
        let given = PathBuf::from(given);

        if !given.join("index.html").is_file() {
            bail!(
                "E2E_DIST is set to {}, which holds no index.html",
                given.display()
            );
        }

        println!("serving the frontend from {}", given.display());
        return Ok(given);
    }

    let root = workspace_root()?.join(dir);

    let dist = target_dir()?.join("e2e-dist");

    println!("building the frontend into {}", dist.display());

    let status = Command::new("trunk")
        .current_dir(&root)
        .arg("build")
        .arg("--dist")
        .arg(&dist)
        .status()
        .await
        .context("running `trunk build`, is trunk installed?")?;

    if !status.success() {
        bail!("`trunk build` failed: {status}");
    }

    Ok(dist)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{is_root, package, search};

    /// A tree shaped like a workspace: the manifest declaring it, and a
    /// package nested under it with a manifest of its own.
    fn workspace(at: &Path) {
        fs::write(
            at.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .expect("writing the manifest");
        fs::create_dir_all(at.join("crates/app/src")).expect("making the crates");
        fs::write(
            at.join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\n",
        )
        .expect("writing a package manifest");
    }

    /// **The root is found from inside a package**, past the package's own
    /// manifest, which is the whole of what the search is for.
    #[test]
    fn the_root_is_found_from_somewhere_under_it() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = dir.path();

        workspace(at);

        assert_eq!(search(&at.join("crates/app/src")).as_deref(), Some(at));
        assert_eq!(search(&at.join("crates/app")).as_deref(), Some(at));

        let deep = at.join("crates/pod/macros/src");
        fs::create_dir_all(&deep).expect("making a nested crate");
        assert_eq!(search(&deep).as_deref(), Some(at));
    }

    /// The root is its own answer, since a walk begins where it is standing.
    #[test]
    fn the_root_is_found_from_the_root() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = dir.path();

        workspace(at);

        assert_eq!(search(at).as_deref(), Some(at));
    }

    /// **Nothing found is an answer rather than a panic**, because the caller
    /// has a sentence to say about it.
    #[test]
    fn somewhere_outside_a_workspace_finds_nothing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = dir.path();

        fs::create_dir_all(at.join("elsewhere")).expect("making a directory");

        assert_eq!(search(&at.join("elsewhere")), None);
        assert_eq!(package(&at.join("elsewhere")), None);
    }

    /// **A package manifest is not a workspace**, since every package has
    /// one; but a package that is not in a workspace is its own root.
    #[test]
    fn a_lone_package_is_its_own_root() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = dir.path();

        fs::create_dir_all(at.join("src")).expect("making a directory");
        fs::write(at.join("Cargo.toml"), "[package]\nname = \"app\"\n")
            .expect("writing a manifest");

        assert!(!is_root(at));
        assert_eq!(search(&at.join("src")), None);
        assert_eq!(package(&at.join("src")).as_deref(), Some(at));
    }

    /// A `Cargo.toml` that is a *directory* is not a manifest.
    #[test]
    fn a_directory_called_cargo_toml_is_not_a_manifest() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = dir.path();

        fs::create_dir_all(at.join("Cargo.toml")).expect("making a directory");

        assert!(!is_root(at));
    }
}
