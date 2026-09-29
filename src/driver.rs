//! Which browser the suite drives, and the WebDriver server that drives it.

use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
use thirtyfour::common::capabilities::firefox::FirefoxPreferences;
use thirtyfour::prelude::*;
use tokio::time::{Instant, timeout};

pub(crate) const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const QUIT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const REAP_TIMEOUT: Duration = Duration::from_secs(5);

/// Cancellation must not leave a browser behind. On Unix the driver starts a
/// new process group, so the kill reaches only descendants of this test's
/// driver, never a person's existing browser. We reap the direct driver child;
/// adoption and reaping of its terminated descendants belongs to the OS.
/// Windows currently has only the
/// child handle fallback; descendant job ownership is separate platform work.
struct OwnedProcess {
    child: tokio::process::Child,
    #[cfg(unix)]
    group: Option<i32>,
}

impl OwnedProcess {
    fn spawn(command: &mut tokio::process::Command) -> Result<Self> {
        command.kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let child = command.spawn()?;
        Ok(Self {
            #[cfg(unix)]
            group: child.id().and_then(|pid| i32::try_from(pid).ok()),
            child,
        })
    }

    fn stop(&mut self) -> Result<()> {
        #[cfg(unix)]
        if let Some(group) = self.group.take() {
            // This is the positive PID returned by our process_group(0) spawn.
            // Forget it before reaping, so Drop cannot target a reused group.
            ensure!(group > 0, "invalid owned driver process group");
            if unsafe { libc::kill(-group, libc::SIGKILL) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error).context("terminating the test driver process group");
                }
            }
        }
        self.child
            .start_kill()
            .context("terminating the test driver")
    }

    async fn reap(&mut self) -> Result<()> {
        let stopped = self.stop();
        let reaped = timeout(REAP_TIMEOUT, self.child.wait())
            .await
            .context("test driver reap timed out")?
            .context("reaping the test driver");
        stopped?;
        reaped?;
        Ok(())
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::warn!(%error, "stopping a dropped test driver");
        }
        // Tokio's kill_on_drop child guard also arranges orphan reaping when
        // an async startup/cleanup future is cancelled before wait completes.
    }
}

/// A browser this suite knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Firefox,
    Chrome,
}

/// Which colour scheme `E2E_THEME` asks the browser to report, or `None` where
/// it says nothing and the machine answers for itself. Without it every run is
/// read in whatever the machine prefers, and the other scheme is never looked
/// at.
fn forced_dark() -> Option<bool> {
    match std::env::var("E2E_THEME").ok()?.as_str() {
        "dark" => Some(true),
        "light" => Some(false),
        _ => None,
    }
}

fn motion_preference(value: &str) -> Result<bool> {
    match value {
        "reduce" => Ok(true),
        "normal" => Ok(false),
        _ => bail!("E2E_MOTION must be reduce or normal, got {value:?}"),
    }
}

fn forced_motion() -> Result<Option<bool>> {
    match std::env::var("E2E_MOTION") {
        Ok(value) => motion_preference(&value).map(Some),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).context("reading E2E_MOTION"),
    }
}

/// The device pixel ratio `E2E_SCALE` asks the browser to report, or `None`
/// where it says nothing and a session runs at the platform's own ratio --
/// ordinarily 1, so a high-DPI-specific path is never actually exercised
/// without this. Unlike `E2E_THEME`/`E2E_MOTION`, both engines answer it:
/// Firefox through a preference, Chrome through a launch flag.
fn forced_scale() -> Result<Option<f64>> {
    match std::env::var("E2E_SCALE") {
        Ok(value) => {
            let scale: f64 = value
                .parse()
                .with_context(|| format!("E2E_SCALE must be a number, got {value:?}"))?;
            ensure!(scale > 0.0, "E2E_SCALE must be positive, got {scale}");
            Ok(Some(scale))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).context("reading E2E_SCALE"),
    }
}

impl Engine {
    /// Every engine, in the order a run with no preference tries them.
    pub const ALL: [Engine; 2] = [Engine::Firefox, Engine::Chrome];

    /// The WebDriver server that drives this engine.
    pub fn driver(self) -> &'static str {
        match self {
            Engine::Firefox => "geckodriver",
            Engine::Chrome => "chromedriver",
        }
    }

    /// What to call this engine where somebody reads it.
    pub fn name(self) -> &'static str {
        match self {
            Engine::Firefox => "firefox",
            Engine::Chrome => "chrome",
        }
    }

    /// The capabilities a session is opened with, headless unless asked
    /// otherwise.
    pub fn capabilities(self, headed: bool) -> Result<Capabilities> {
        self.with_preferences(headed, forced_dark(), forced_motion()?, forced_scale()?)
    }

    fn with_preferences(
        self,
        headed: bool,
        dark: Option<bool>,
        reduced_motion: Option<bool>,
        scale: Option<f64>,
    ) -> Result<Capabilities> {
        match self {
            Engine::Firefox => {
                let mut caps = DesiredCapabilities::firefox();

                if !headed {
                    caps.add_arg("--headless")?;
                }

                if dark.is_some() || reduced_motion.is_some() || scale.is_some() {
                    let mut prefs = FirefoxPreferences::new();
                    if let Some(dark) = dark {
                        prefs.set("ui.systemUsesDarkTheme", i64::from(dark))?;
                    }
                    if let Some(reduced) = reduced_motion {
                        prefs.set("ui.prefersReducedMotion", i64::from(reduced))?;
                    }
                    if let Some(scale) = scale {
                        prefs.set("layout.css.devPixelsPerPx", scale.to_string())?;
                    }
                    caps.set_preferences(prefs)?;
                }

                Ok(caps.into())
            }
            Engine::Chrome => {
                let mut caps = DesiredCapabilities::chrome();

                if !headed {
                    caps.add_arg("--headless=new")?;
                }

                if dark.is_some() {
                    bail!("E2E_THEME is a Firefox preference and chrome has no answer for it");
                }
                if reduced_motion.is_some() {
                    bail!("E2E_MOTION is a Firefox preference and chrome has no answer for it");
                }

                if let Some(scale) = scale {
                    caps.add_arg(&format!("--force-device-scale-factor={scale}"))?;
                    caps.add_arg("--high-dpi-support=1")?;
                }

                Ok(caps.into())
            }
        }
    }

    /// Where this engine's driver already is, or `None` for a machine that has
    /// not got one.
    pub(crate) fn installed(self) -> Option<PathBuf> {
        if let Some(path) = on_path(self.driver()) {
            return Some(path);
        }

        let cached = crate::fetch::cached(self).ok()?;
        cached.is_file().then_some(cached)
    }

    /// Whether this engine can be driven without fetching anything.
    pub fn available(self) -> bool {
        self.installed().is_some()
    }

    /// Whether this engine's browser is here, whether or not its driver is.
    pub fn browser_present(self) -> bool {
        match self {
            Engine::Firefox => false,
            Engine::Chrome => crate::fetch::chrome_present(),
        }
    }

    /// The first engine whose driver this machine has — failing that, the first
    /// whose *browser* it has, which a fetch can finish.
    pub fn detect() -> Option<Engine> {
        Engine::ALL
            .into_iter()
            .find(|engine| engine.available())
            .or_else(|| {
                Engine::ALL
                    .into_iter()
                    .find(|engine| engine.browser_present())
            })
    }

    /// [`Engine::detect`], or an error naming what would have satisfied it.
    pub fn require() -> Result<Engine> {
        if let Some(engine) = Engine::detect() {
            return Ok(engine);
        }

        let drivers = Engine::ALL
            .map(|engine| format!("{} (for {})", engine.driver(), engine.name()))
            .join(", ");

        bail!(
            "No browser this suite can drive. It needs either a WebDriver server \
             on PATH — one of: {drivers} — or a Chrome it can fetch a matching \
             chromedriver for, which it does by itself into target/e2e."
        )
    }

    /// Where this engine's driver is, fetching one if that is the only way.
    pub(crate) fn provision(self) -> Result<PathBuf> {
        if let Some(path) = self.installed() {
            return Ok(path);
        }

        crate::fetch::fetch(self)
    }
}

impl fmt::Display for Engine {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Engine {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        for engine in Engine::ALL {
            if input.eq_ignore_ascii_case(engine.name()) {
                return Ok(engine);
            }
        }

        match input.to_ascii_lowercase().as_str() {
            "gecko" => Ok(Engine::Firefox),
            "chromium" => Ok(Engine::Chrome),
            _ => {
                let names = Engine::ALL.map(Engine::name).join(", ");
                Err(anyhow!(
                    "{input:?} is not a browser this suite drives: {names}"
                ))
            }
        }
    }
}

/// Where `name` is on `PATH`, taking the platform's executable suffix into
/// account.
fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    let suffixes: Vec<String> = match cfg!(windows) {
        true => std::env::var("PATHEXT")
            .unwrap_or_else(|_| String::from(".EXE"))
            .split(';')
            .map(str::to_ascii_lowercase)
            .chain([String::new()])
            .collect(),
        false => vec![String::new()],
    };

    for dir in std::env::split_paths(&path) {
        for suffix in &suffixes {
            let candidate = dir.join(format!("{name}{suffix}"));

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// A running WebDriver server, and the address it answers on.
pub(crate) struct Driver {
    engine: Engine,
    process: OwnedProcess,
    stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
    stderr: tokio::io::BufReader<tokio::process::ChildStderr>,
    stdout_line: Vec<u8>,
    stderr_line: Vec<u8>,
    stdout_open: bool,
    stderr_open: bool,
    log: String,
    pub(crate) address: SocketAddr,
}

impl Driver {
    /// Start one, and do not return until it says where it is answering.
    pub(crate) async fn new(engine: Engine, deadline: Instant) -> Result<Self> {
        use tokio::process::Command;

        let binary = engine.installed().with_context(|| {
            anyhow!(
                "No {} to drive {engine} with. It is provisioned once before the \
                 run starts; see `harness::choose`.",
                engine.driver()
            )
        })?;

        let mut command = Command::new(&binary);
        command.args(engine.port_args());
        Self::start(engine, &mut command, deadline).await
    }

    pub(crate) async fn start(
        engine: Engine,
        command: &mut tokio::process::Command,
        deadline: Instant,
    ) -> Result<Self> {
        use std::process::Stdio;
        use tokio::io::BufReader;

        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let mut process = OwnedProcess::spawn(command).context("spawning the test driver")?;
        let stdout = BufReader::new(process.child.stdout.take().context("missing stdout")?);
        let stderr = BufReader::new(process.child.stderr.take().context("missing stderr")?);
        let mut driver = Self {
            engine,
            process,
            stdout,
            stderr,
            stdout_line: Vec::new(),
            stderr_line: Vec::new(),
            stdout_open: true,
            stderr_open: true,
            log: String::new(),
            address: SocketAddr::from(([127, 0, 0, 1], 0)),
        };
        let ready = tokio::time::timeout_at(deadline, async {
            for _ in 0..READY_LINES {
                ensure!(
                    driver.io().await?,
                    "driver stopped before announcing its address"
                );
                if let Some(address) = driver.log.lines().find_map(|line| engine.address_of(line)) {
                    driver.address = address;
                    return Ok(());
                }
            }
            bail!("driver did not announce its address in its first {READY_LINES} lines")
        })
        .await
        .context("test browser startup timed out while waiting for the driver address")
        .and_then(|result| result);
        if let Err(error) = ready {
            let error = error.context(driver.diagnostics());
            return Err(with_cleanup(error, driver.terminate().await));
        }
        Ok(driver)
    }

    /// Which engine this is driving, for whatever reads it back.
    #[inline]
    pub(crate) fn engine(&self) -> Engine {
        self.engine
    }

    #[inline]
    pub(crate) async fn io(&mut self) -> Result<bool> {
        use tokio::io::AsyncBufReadExt;
        while self.stdout_open || self.stderr_open {
            let (n, stderr) = tokio::select! {
                n = self.stdout.read_until(b'\n', &mut self.stdout_line), if self.stdout_open => (n?, false),
                n = self.stderr.read_until(b'\n', &mut self.stderr_line), if self.stderr_open => (n?, true),
            };
            if n == 0 {
                if stderr {
                    self.stderr_open = false;
                } else {
                    self.stdout_open = false;
                }
                continue;
            }
            // Keep each pipe's partial line across select cancellation.
            let bytes = if stderr {
                &mut self.stderr_line
            } else {
                &mut self.stdout_line
            };
            let line = String::from_utf8_lossy(bytes);
            self.log.push_str(&line);
            if self.log.len() > 8192 {
                let mut at = self.log.len() - 8192;
                while !self.log.is_char_boundary(at) {
                    at += 1;
                }
                self.log.drain(..at);
            }
            tracing::trace!(driver = self.engine.driver(), stderr, line = %line.trim(), "test driver output");
            bytes.clear();
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn diagnostics(&self) -> String {
        format!("{}{}", self.engine.driver(), quoted(&self.log))
    }

    /// Drain both pipes while a protocol operation is pending: a full stderr
    /// pipe during browser boot must not masquerade as a hung session request.
    pub(crate) async fn during<T>(
        &mut self,
        deadline: Instant,
        stage: &str,
        operation: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        let mut operation = std::pin::pin!(operation);
        let result = loop {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => break Err(TimedOut(stage.to_owned()).into()),
                result = operation.as_mut() => break result,
                read = self.io(), if self.stdout_open || self.stderr_open => {
                    if let Err(error) = read { break Err(error); }
                    // A driver may close logging pipes before replying to quit.
                    // Keep polling the protocol, still under the same deadline.
                }
            }
        };
        result.with_context(|| format!("browser {stage}: {}", self.diagnostics()))
    }

    pub(crate) async fn terminate(&mut self) -> Result<()> {
        self.process.reap().await
    }
}

/// A browser operation that did not answer by its deadline, named by stage.
#[derive(Debug)]
pub(crate) struct TimedOut(pub(crate) String);

impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "test browser {} timed out", self.0)
    }
}

impl std::error::Error for TimedOut {}

pub(crate) fn with_cleanup(error: anyhow::Error, cleanup: Result<()>) -> anyhow::Error {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => error.context(format!("test browser cleanup also failed: {cleanup:#}")),
    }
}

/// How many lines of a driver's greeting are read while waiting for it to say it
/// is listening. chromedriver takes four; the rest is headroom for a version
/// that says more.
const READY_LINES: usize = 16;

impl Engine {
    /// How this engine's driver is told to serve on a port of its own choosing.
    fn port_args(self) -> &'static [&'static str] {
        match self {
            Engine::Firefox => &["--port", "0"],
            Engine::Chrome => &["--port=0"],
        }
    }

    /// The address a line of the driver's greeting announces, if it is the line
    /// that announces one.
    fn address_of(self, line: &str) -> Option<SocketAddr> {
        match self {
            Engine::Firefox => {
                let at = line.find("Listening on ")? + "Listening on ".len();
                line[at..].trim().parse().ok()
            }
            Engine::Chrome => {
                const SAID: &str = "started successfully on port ";

                let at = line.find(SAID)? + SAID.len();
                let port: u16 = line[at..].trim().trim_end_matches('.').parse().ok()?;

                Some(SocketAddr::from(([127, 0, 0, 1], port)))
            }
        }
    }
}

/// Whatever the driver managed to say, set off so it reads as its words rather
/// than as this suite's.
fn quoted(said: &str) -> String {
    match said.trim() {
        "" => String::from(" It printed nothing."),
        said => format!(" It said:\n{said}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, motion_preference};

    #[test]
    fn motion_preferences_are_explicit_and_do_not_replace_theme_preferences() {
        assert!(motion_preference("reduce").unwrap());
        assert!(!motion_preference("normal").unwrap());
        for invalid in ["", "false", "off", "reduced"] {
            assert!(motion_preference(invalid).is_err());
        }
        for reduced in [false, true] {
            let caps = Engine::Firefox
                .with_preferences(false, Some(true), Some(reduced), None)
                .unwrap();
            let prefs = &caps.get("moz:firefoxOptions").unwrap()["prefs"];
            assert_eq!(prefs["ui.systemUsesDarkTheme"], 1);
            assert_eq!(prefs["ui.prefersReducedMotion"], i64::from(reduced));
            let error = Engine::Chrome
                .with_preferences(false, None, Some(reduced), None)
                .unwrap_err();
            assert!(error.to_string().contains("E2E_MOTION"));
        }
        let caps = Engine::Firefox
            .with_preferences(false, None, None, None)
            .unwrap();
        assert!(
            caps.get("moz:firefoxOptions")
                .unwrap()
                .get("prefs")
                .is_none()
        );
        assert!(
            Engine::Chrome
                .with_preferences(false, None, None, None)
                .is_ok()
        );
    }

    /// **A forced device pixel ratio reaches both engines**, unlike theme and
    /// motion, which are Firefox-only preferences.
    #[test]
    fn a_forced_scale_reaches_both_engines() {
        let caps = Engine::Firefox
            .with_preferences(false, None, None, Some(2.0))
            .unwrap();
        let prefs = &caps.get("moz:firefoxOptions").unwrap()["prefs"];
        assert_eq!(prefs["layout.css.devPixelsPerPx"], "2");

        let caps = Engine::Chrome
            .with_preferences(false, None, None, Some(2.0))
            .unwrap();
        let args = caps.get("goog:chromeOptions").unwrap()["args"]
            .as_array()
            .unwrap();
        assert!(args.iter().any(|a| a == "--force-device-scale-factor=2"));
    }

    /// **Every engine is named, parses back, and has a driver of its own.** The
    /// list is short enough that this looks like ceremony and is not: a third
    /// entry added with a copied `name` would make `--browser` reach the wrong
    /// one, silently, and the suite would report the engine it was not running.
    #[test]
    fn every_engine_round_trips_through_its_name() {
        for engine in Engine::ALL {
            assert_eq!(engine.name().parse::<Engine>().unwrap(), engine);

            assert_eq!(
                engine.name().to_uppercase().parse::<Engine>().unwrap(),
                engine
            );
        }

        for pair in Engine::ALL.windows(2) {
            assert_ne!(pair[0].name(), pair[1].name());
            assert_ne!(pair[0].driver(), pair[1].driver());
        }
    }

    /// A name nothing answers to says what would have worked, rather than
    /// failing somewhere further in with a spawn error naming one binary.
    #[test]
    fn a_browser_nothing_drives_is_refused_with_the_list() {
        let error = "safari".parse::<Engine>().unwrap_err().to_string();

        for engine in Engine::ALL {
            assert!(
                error.contains(engine.name()),
                "{error} does not offer {engine}"
            );
        }
    }

    /// **Each driver's greeting is read for the port it took, and only the line
    /// that carries one counts.**
    #[test]
    fn each_driver_is_read_for_the_port_it_took() {
        use std::net::SocketAddr;

        let gecko = "1730000000\tgeckodriver\tINFO\tListening on 127.0.0.1:51234";

        let chrome = [
            "Starting ChromeDriver 151.0.7922.138 (abcdef) on port 0",
            "Only local connections are allowed.",
            "Please see https://chromedriver.chromium.org/security-considerations \
             for suggestions on keeping ChromeDriver safe.",
            "ChromeDriver was started successfully on port 52725.",
        ];

        let expect = |port| Some(SocketAddr::from(([127, 0, 0, 1], port)));

        assert_eq!(Engine::Firefox.address_of(gecko), expect(51234));
        assert_eq!(Engine::Chrome.address_of(gecko), None);

        assert_eq!(Engine::Chrome.address_of(chrome[3]), expect(52725));

        for line in &chrome[..3] {
            assert_eq!(
                Engine::Chrome.address_of(line),
                None,
                "{line:?} is not where the port is"
            );
            assert_eq!(Engine::Firefox.address_of(line), None);
        }
    }

    /// **Both are told to pick a port, which is what makes starting several at
    /// once safe** — and the two spellings are not interchangeable.
    /// chromedriver answers a separated `--port 0` with its usage and an exit.
    #[test]
    fn each_driver_is_told_to_pick_its_own_port() {
        for engine in Engine::ALL {
            let args = engine.port_args();

            assert!(
                args.iter().any(|arg| arg.contains('0')),
                "{engine} is not asking for a port of the driver's choosing"
            );
        }

        assert_eq!(Engine::Firefox.port_args(), ["--port", "0"]);
        assert_eq!(Engine::Chrome.port_args(), ["--port=0"]);
    }

    /// The suite asks the machine what it can drive before it tries to drive
    /// anything, so a machine with neither driver is told what to install
    /// instead of being handed a spawn error naming one binary.
    #[test]
    fn a_machine_with_no_driver_is_told_what_would_have_worked() {
        if Engine::detect().is_some() {
            assert!(Engine::require().is_ok());
            return;
        }

        let error = Engine::require().unwrap_err().to_string();

        for engine in Engine::ALL {
            assert!(
                error.contains(engine.driver()),
                "{error} does not name a driver"
            );
        }
    }
}

#[cfg(all(test, unix))]
pub(crate) mod lifecycle_tests {
    use super::*;
    use tokio::process::Command;

    pub(crate) async fn fake(address: SocketAddr) -> (Driver, i32) {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            &format!("echo 'Listening on {address}'; exec sleep 60"),
        ]);
        let driver = Driver::start(
            Engine::Firefox,
            &mut command,
            Instant::now() + STARTUP_TIMEOUT,
        )
        .await
        .unwrap();
        let pid = driver.process.child.id().unwrap() as i32;
        (driver, pid)
    }

    pub(crate) async fn gone(pid: i32) {
        timeout(Duration::from_secs(2), async {
            loop {
                let result = unsafe { libc::kill(pid, 0) };
                if result != 0
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the owned driver must be reaped");
    }

    #[tokio::test]
    async fn readiness_timeout_keeps_stderr_and_reaps_its_process() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "echo owned-pid=$$ >&2; echo boot-stalled >&2; exec sleep 60",
        ]);
        let error = Driver::start(
            Engine::Firefox,
            &mut command,
            Instant::now() + Duration::from_millis(250),
        )
        .await
        .err()
        .unwrap();
        let message = format!("{error:#}");
        assert!(message.contains("boot-stalled"), "{message}");
        assert!(message.contains("startup timed out"), "{message}");
        let pid = message
            .lines()
            .find_map(|line| line.strip_prefix("owned-pid="))
            .unwrap()
            .parse()
            .unwrap();
        gone(pid).await;
    }

    #[tokio::test]
    async fn early_driver_exit_reports_its_original_error_and_output() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo fatal-startup-marker >&2; exit 7"]);
        let error = Driver::start(
            Engine::Firefox,
            &mut command,
            Instant::now() + STARTUP_TIMEOUT,
        )
        .await
        .err()
        .unwrap();
        let message = format!("{error:#}");
        assert!(message.contains("fatal-startup-marker"), "{message}");
        assert!(message.contains("stopped before announcing"), "{message}");
    }

    #[tokio::test]
    async fn closing_one_log_pipe_does_not_hide_the_other_pipes_greeting() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "exec 1>&-; echo 'Listening on 127.0.0.1:12345' >&2; exec sleep 60",
        ]);
        let mut driver = Driver::start(
            Engine::Firefox,
            &mut command,
            Instant::now() + STARTUP_TIMEOUT,
        )
        .await
        .unwrap();
        assert_eq!(driver.address.port(), 12345);
        assert!(driver.diagnostics().contains("Listening on"));
        driver.terminate().await.unwrap();
    }

    #[tokio::test]
    async fn cancelling_before_the_driver_announces_its_address_reaps_it() {
        let pid_file = tempfile::NamedTempFile::new().unwrap();
        let path = pid_file.path().to_owned();
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "printf '%s' \"$$\" > \"$1\"; exec sleep 60",
            "test-driver",
        ]);
        command.arg(&path);
        let starting = tokio::spawn(async move {
            Driver::start(
                Engine::Firefox,
                &mut command,
                Instant::now() + STARTUP_TIMEOUT,
            )
            .await
        });
        let pid = timeout(STARTUP_TIMEOUT, async {
            loop {
                if let Ok(pid) = tokio::fs::read_to_string(&path)
                    .await
                    .unwrap()
                    .parse::<i32>()
                {
                    break pid;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        starting.abort();
        assert!(matches!(starting.await, Err(error) if error.is_cancelled()));
        gone(pid).await;
    }

    #[tokio::test]
    async fn killing_an_owned_group_does_not_kill_an_unrelated_child() {
        use tokio::io::AsyncBufReadExt;
        let mut unrelated = Command::new("sleep")
            .arg("60")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 60 & echo $!; wait"]);
        command.stdout(std::process::Stdio::piped());
        let mut owned = OwnedProcess::spawn(&mut command).unwrap();
        let leader = owned.child.id().unwrap() as i32;
        let mut child_line = String::new();
        tokio::io::BufReader::new(owned.child.stdout.take().unwrap())
            .read_line(&mut child_line)
            .await
            .unwrap();
        let descendant: i32 = child_line.trim().parse().unwrap();
        assert_ne!(leader, descendant);
        assert_eq!(unsafe { libc::getpgid(leader) }, leader);
        assert_eq!(unsafe { libc::getpgid(descendant) }, leader);
        assert_ne!(
            unsafe { libc::getpgid(unrelated.id().unwrap() as i32) },
            leader
        );
        owned.reap().await.unwrap();
        gone(leader).await;
        // Grandchildren are reaped by their new parent, not this harness. A
        // zombie has stopped running even if init has not collected it yet.
        timeout(Duration::from_secs(2), async {
            loop {
                let dead = unsafe { libc::kill(descendant, 0) } != 0;
                #[cfg(target_os = "linux")]
                let dead = dead
                    || std::fs::read_to_string(format!("/proc/{descendant}/stat")).is_ok_and(
                        |stat| {
                            stat.rsplit_once(") ")
                                .is_some_and(|(_, fields)| fields.starts_with('Z'))
                        },
                    );
                if dead {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(unrelated.try_wait().unwrap().is_none());
        unrelated.kill().await.unwrap();
    }

    #[test]
    fn cleanup_failure_keeps_the_original_failure_chain() {
        let error = with_cleanup(
            anyhow!("original assertion failed"),
            Err(TimedOut("quit".to_owned()).into()),
        );
        let message = format!("{error:#}");
        assert!(message.contains("original assertion failed"));
        assert!(message.contains("quit timed out"));
    }

    #[test]
    fn a_timeout_stays_recognisable_under_its_context() {
        let error = anyhow::Error::from(TimedOut("quit".to_owned()))
            .context("browser quit: geckodriver It printed nothing.");
        assert!(error.downcast_ref::<TimedOut>().is_some());
        assert!(format!("{error:#}").contains("test browser quit timed out"));
        assert!(
            anyhow!("quit timed out")
                .downcast_ref::<TimedOut>()
                .is_none()
        );
    }
}
