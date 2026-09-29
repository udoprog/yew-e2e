//! Getting hold of a WebDriver server the suite can drive.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::driver::Engine;

/// Where a fetched driver is kept, relative to the workspace root.
const CACHE: &str = "target/e2e";

/// Chrome for Testing's index of what it publishes, keyed by
/// `MAJOR.MINOR.BUILD` and answering with the latest patch of each and every
/// download that goes with it.
const INDEX: &str = "https://googlechromelabs.github.io/chrome-for-testing/latest-patch-versions-per-build-with-downloads.json";

/// What Chrome for Testing calls the platform this is being built for.
const PLATFORM: &str = if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
    "win64"
} else if cfg!(all(target_os = "windows", target_arch = "x86")) {
    "win32"
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    "mac-arm64"
} else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
    "mac-x64"
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    "linux64"
} else {
    ""
};

/// Where a fetched driver for `engine` is, whether or not it is there yet.
pub(crate) fn cached(engine: Engine) -> Result<PathBuf> {
    let name = match cfg!(windows) {
        true => format!("{}.exe", engine.driver()),
        false => engine.driver().to_owned(),
    };

    Ok(crate::dist::workspace_root()?.join(CACHE).join(name))
}

/// Fetch a driver for `engine` into [`cached`], and answer where it went.
pub(crate) fn fetch(engine: Engine) -> Result<PathBuf> {
    if engine != Engine::Chrome {
        bail!(
            "{} is not fetched: it drives every version of its browser, so a \
             machine that has the browser needs it installed once and never \
             matched. Install it and put it on PATH.",
            engine.driver()
        );
    }

    if PLATFORM.is_empty() {
        bail!(
            "Chrome for Testing publishes no chromedriver for {} on {}. Install \
             one by hand and put it on PATH.",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }

    let installed = chrome_version()?;
    let build = build_of(&installed)?;

    tracing::info!("Chrome {installed} is installed; looking for a chromedriver for {build}");

    let url = download_url(&build, &installed)?;

    tracing::info!("Fetching {url}");

    let archive = reqwest::blocking::get(&url)
        .and_then(reqwest::blocking::Response::error_for_status)
        .with_context(|| anyhow!("Fetching {url}"))?
        .bytes()
        .with_context(|| anyhow!("Reading {url}"))?;

    let at = cached(engine)?;

    let dir = at.parent().context("the cache has a parent")?;
    fs::create_dir_all(dir).with_context(|| anyhow!("Creating {}", dir.display()))?;

    extract(&archive, engine.driver(), &at)?;

    tracing::info!("chromedriver {installed} is at {}", at.display());

    Ok(at)
}

/// Pull the one file out of the archive and put it where it belongs.
fn extract(archive: &[u8], name: &str, at: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).context("Reading the archive")?;

    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;

        let Some(entry_name) = entry.enclosed_name() else {
            continue;
        };

        let Some(stem) = entry_name.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        if stem != name {
            continue;
        }

        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;

        fs::write(at, &bytes).with_context(|| anyhow!("Writing {}", at.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(at, fs::Permissions::from_mode(0o755))?;
        }

        return Ok(());
    }

    bail!("The archive holds no {name}")
}

/// Whether this machine has a Chrome at all.
pub(crate) fn chrome_present() -> bool {
    chrome_version().is_ok()
}

/// The version of the Chrome this machine has.
fn chrome_version() -> Result<String> {
    #[cfg(windows)]
    {
        windows_chrome_version()
    }

    #[cfg(not(windows))]
    {
        unix_chrome_version()
    }
}

/// **Read off the install directory rather than asked of the browser.** On
/// Windows `chrome.exe --version` prints nothing anywhere a pipe can catch it —
/// it is a GUI subsystem binary, so it detaches and the shell gets an empty
/// stdout and a success. What Chrome does have is a directory named for the
/// version it is currently on, beside the executable, which the updater
/// maintains.
#[cfg(windows)]
fn windows_chrome_version() -> Result<String> {
    let mut tried = Vec::new();

    for root in chrome_roots() {
        tried.push(root.display().to_string());

        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };

        let mut best: Option<String> = None;

        for entry in entries.flatten() {
            let name = entry.file_name();

            let Some(name) = name.to_str() else {
                continue;
            };

            if !is_version(name) || !entry.path().is_dir() {
                continue;
            }

            if best.as_deref().is_none_or(|had| older(had, name)) {
                best = Some(name.to_owned());
            }
        }

        if let Some(version) = best {
            return Ok(version);
        }
    }

    bail!(
        "No Chrome found. Looked in: {}. The suite needs a browser to drive; \
         install Chrome, or install geckodriver and run with --browser firefox.",
        tried.join(", ")
    )
}

/// Everywhere Chrome installs itself on Windows, per-machine and per-user.
#[cfg(windows)]
fn chrome_roots() -> Vec<PathBuf> {
    const SUFFIX: &str = r"Google\Chrome\Application";

    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(SUFFIX))
        .collect()
}

/// Everywhere else, where the browser answers for itself.
#[cfg(not(windows))]
fn unix_chrome_version() -> Result<String> {
    const NAMES: [&str; 4] = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ];

    for name in NAMES {
        let Ok(output) = std::process::Command::new(name).arg("--version").output() else {
            continue;
        };

        if !output.status.success() {
            continue;
        }

        let line = String::from_utf8_lossy(&output.stdout);

        if let Some(version) = line.split_whitespace().find(|word| is_version(word)) {
            return Ok(version.to_owned());
        }
    }

    bail!(
        "No Chrome found: tried {}. The suite needs a browser to drive; install \
         Chrome, or install geckodriver and run with --browser firefox.",
        NAMES.join(", ")
    )
}

/// Whether a word is a Chrome version: four numbers separated by dots.
fn is_version(word: &str) -> bool {
    let mut parts = 0;

    for part in word.split('.') {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }

        parts += 1;
    }

    parts == 4
}

/// Whether `a` is an earlier version than `b`, compared number by number.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(dead_code, reason = "used where Chrome will not say its own version")
)]
fn older(a: &str, b: &str) -> bool {
    let numbers = |v: &str| {
        v.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };

    numbers(a) < numbers(b)
}

/// The `MAJOR.MINOR.BUILD` a full version belongs to, which is how Chrome for
/// Testing keys what it publishes.
fn build_of(version: &str) -> Result<String> {
    let mut parts = version.split('.');

    let (Some(major), Some(minor), Some(build)) = (parts.next(), parts.next(), parts.next()) else {
        bail!("{version:?} is not a Chrome version");
    };

    Ok(format!("{major}.{minor}.{build}"))
}

/// Ask Chrome for Testing where the chromedriver for this build is.
fn download_url(build: &str, installed: &str) -> Result<String> {
    let index: serde_json::Value = reqwest::blocking::get(INDEX)
        .and_then(reqwest::blocking::Response::error_for_status)
        .with_context(|| anyhow!("Fetching {INDEX}"))?
        .json()
        .with_context(|| anyhow!("Reading {INDEX}"))?;

    let entry = index
        .get("builds")
        .and_then(|builds| builds.get(build))
        .with_context(|| {
            anyhow!(
                "Chrome for Testing publishes nothing for build {build} (Chrome \
                 {installed} is installed). A Chrome newer than the index is the \
                 usual reason; a driver installed by hand and put on PATH is \
                 used in preference to anything fetched."
            )
        })?;

    let downloads = entry
        .get("downloads")
        .and_then(|downloads| downloads.get("chromedriver"))
        .and_then(serde_json::Value::as_array)
        .with_context(|| anyhow!("Build {build} publishes a Chrome but no chromedriver"))?;

    for download in downloads {
        if download.get("platform").and_then(serde_json::Value::as_str) == Some(PLATFORM) {
            let url = download
                .get("url")
                .and_then(serde_json::Value::as_str)
                .context("a download has a url")?;

            return Ok(url.to_owned());
        }
    }

    bail!("Build {build} publishes no chromedriver for {PLATFORM}")
}

#[cfg(test)]
mod tests {
    use super::{build_of, is_version, older};

    /// **The shape test decides what gets turned into a download URL**, since it
    /// is what picks Chrome's version out of a directory listing that also holds
    /// files and `SetupMetrics`. Loose, it would name one of those.
    #[test]
    fn only_a_four_part_number_is_a_version() {
        assert!(is_version("151.0.7922.174"));
        assert!(is_version("99.0.0.0"));

        for not in [
            "151.0.7922",
            "151.0.7922.174.1",
            "SetupMetrics",
            "151.0.7922.x",
            "chrome.exe",
            "",
            "...",
            "151..7922.174",
        ] {
            assert!(!is_version(not), "{not:?} was taken for a version");
        }
    }

    /// **Numbers, not text.** Chrome's patch number runs past 99 within a build,
    /// so a lexicographic pick would leave a machine on `…99` while it is running
    /// `…138` — a major-version match by luck rather than by construction.
    #[test]
    fn the_newest_install_is_picked_by_number() {
        assert!(older("151.0.7922.99", "151.0.7922.138"));
        assert!(!older("151.0.7922.138", "151.0.7922.99"));

        assert!(older("151.0.7922.174", "152.0.7977.54"));
        assert!(!older("151.0.7922.174", "151.0.7922.174"));
    }

    /// The key Chrome for Testing publishes under is the build, which is the
    /// version with its patch taken off.
    #[test]
    fn a_version_names_the_build_it_belongs_to() {
        assert_eq!(build_of("151.0.7922.174").unwrap(), "151.0.7922");
        assert_eq!(build_of("113.0.5672.63").unwrap(), "113.0.5672");

        assert!(build_of("151.0").is_err());
    }
}
