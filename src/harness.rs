//! The suite's own runner, in place of libtest.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::future::Future;
use std::io::Write as _;
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser};
use futures_util::stream::{FuturesUnordered, StreamExt};

use crate::Fixture;
use crate::driver::Engine;
use crate::sessions::{self, Outcome};

/// One test's own future, borrowing the browser and the fixture it was handed.
pub type TestFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + 'a>>;

/// How the runner starts one test.
pub type Start<F> = for<'a> fn(&'a mut crate::TestDriver, &'a mut F) -> TestFuture<'a>;

/// Whether the browser is drawn where somebody can watch it.
static HEADED: OnceLock<bool> = OnceLock::new();

/// Which browser this run drives, decided once for the same reason.
static ENGINE: OnceLock<Engine> = OnceLock::new();

/// Whether this run shows the browser.
pub(crate) fn headed() -> bool {
    if let Some(&headed) = HEADED.get() {
        return headed;
    }

    std::env::var_os("E2E_HEADED").is_some_and(|value| value != "0" && value != "false")
}

/// Which browser this run drives.
pub fn engine() -> Engine {
    match ENGINE.get() {
        Some(&engine) => engine,
        None => Engine::detect().unwrap_or(Engine::Firefox),
    }
}

/// What `--browser` and `E2E_BROWSER` between them say, or detection where
/// neither does.
fn choose(named: Option<Engine>) -> Result<Engine> {
    let named = match named {
        Some(engine) => Some(engine),
        None => match std::env::var("E2E_BROWSER") {
            Ok(value) => Some(value.parse::<Engine>()?),
            Err(_) => None,
        },
    };

    let Some(engine) = named else {
        return Engine::require();
    };

    if !engine.available() && !engine.browser_present() {
        anyhow::bail!(
            "--browser {engine} needs {driver}, and neither it nor a {engine} to \
             fetch one for is on this machine.",
            driver = engine.driver()
        );
    }

    Ok(engine)
}

/// The tests this run may choose from, in the order they were declared.
pub struct Suite<F: Fixture> {
    tests: Vec<Test<F>>,
    config: crate::Config,
}

struct Test<F: Fixture> {
    name: String,
    start: Start<F>,
    setup: F::Setup,
}

impl<F: Fixture> Default for Suite<F> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Fixture> Suite<F> {
    #[inline]
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            config: F::config(),
        }
    }

    /// Declare one test.
    pub fn add(&mut self, name: impl Into<String>, start: Start<F>, setup: F::Setup) {
        self.tests.push(Test {
            name: name.into(),
            start,
            setup,
        });
    }

    /// Read the arguments, run what they name, and report.
    pub fn run(self) -> ! {
        let parsed = Options::command()
            .name(program().leak() as &str)
            .about(self.config.about.clone())
            .long_about(format!(
                "{}\n\n\
                 Also read: E2E_HEADED, which --headed overrides, E2E_BROWSER, which \
                 --browser overrides, E2E_DIST to serve a frontend that is already built \
                 rather than building one, E2E_THEME, E2E_MOTION and E2E_SCALE for what \
                 the browser reports, E2E_SHOTS for where snapshots go, and the usual \
                 RUST_LOG.",
                self.config.about
            ))
            .get_matches();

        let options = match Options::from_arg_matches(&parsed) {
            Ok(options) => options,
            Err(error) => error.exit(),
        };

        let code = match self.try_run(options) {
            Ok(true) => 0,
            Ok(false) => 1,
            Err(error) => {
                eprintln!("error: {error:?}");
                2
            }
        };

        std::process::exit(code)
    }

    fn try_run(self, options: Options) -> Result<bool> {
        // Wait before reading --last-session too: the preceding run may still
        // be writing it. Listing is read-only and should never queue a build.
        let _run_lock = if options.list {
            None
        } else {
            let wait = match options.lock_wait {
                0 => None,
                seconds => Some(Duration::from_secs(u64::from(seconds))),
            };

            let directory = crate::run_lock::directory(options.lock)?;
            crate::run_lock::acquire(directory.as_deref(), &self.config.lock, wait)?
        };

        sessions::name_dir(&self.config.sessions);

        let picked = match (options.last_session, options.session.as_deref()) {
            (true, _) => Some(sessions::latest()?),
            (false, Some(name)) => Some(sessions::load(name)?),
            (false, None) => None,
        };

        let mut chosen = self
            .tests
            .into_iter()
            .filter(|test| options.wants(&test.name))
            .collect::<Vec<_>>();

        // A listing is somebody else's input, so under --list the commentary
        // goes to stderr and stdout stays nothing but the names.
        let mut prose = commentary(options.list);

        if let Some(session) = &picked {
            let left = session.unfinished();
            let (passed, failed, missed) = session.counts();

            chosen.retain(|test| left.contains(&test.name.as_str()));

            _ = writeln!(
                prose,
                "picking up {}: {passed} passed, {failed} failed, {missed} not run",
                session.name
            );

            for (name, message, seconds) in session.failures() {
                if !chosen.iter().any(|test| test.name == name) {
                    continue;
                }

                // How long it took is beside how it ended on purpose: a failure
                // that took the whole cap is a different thing to read than one
                // that took a second, and the two are only worth telling apart
                // where both are in front of you.
                match seconds {
                    Some(seconds) => {
                        _ = writeln!(prose, "\n---- {name} failed there after {seconds:.1}s ----");
                    }
                    None => _ = writeln!(prose, "\n---- {name} failed there ----"),
                }

                for line in message.lines() {
                    _ = writeln!(prose, "    {line}");
                }

                _ = writeln!(prose);
            }

            if chosen.is_empty() {
                _ = writeln!(prose, "nothing left to run in {}", session.name);
            }
        }

        if options.list {
            for test in &chosen {
                println!("{}: test", test.name);
            }

            return Ok(true);
        }

        if chosen.is_empty() {
            anyhow::ensure!(
                options.allow_empty,
                "no tests selected by the filter — this is not a pass; the executable may be stale"
            );
            return Ok(true);
        }

        let engine = choose(options.browser)?;

        _ = HEADED.set(options.headed);
        _ = ENGINE.set(engine);
        F::init_tracing();

        let driver = engine.provision()?;
        tracing::info!("Driving {engine} with {}", driver.display());

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let sandbox = crate::sandbox::Sandbox::enter(F::enter_sandbox)?;

        let frontend = self.config.frontend;

        let ok = runtime.block_on(async move {
            crate::dist::build(&frontend).await?;
            drive(chosen, &options).await
        })?;

        if let Err(error) = sandbox.finish() {
            println!(
                "
error: {error:?}"
            );
            return Ok(false);
        }

        Ok(ok)
    }
}

/// Run `tests`, at most [`Options::parallel`] of them at once, reporting each as
/// it finishes.
async fn drive<F: Fixture>(tests: Vec<Test<F>>, options: &Options) -> Result<bool> {
    let started = Instant::now();
    let total = tests.len();

    let session = sessions::Session::new(tests.iter().map(|test| test.name.clone()));

    println!(
        "running {total} test{} as {}",
        if total == 1 { "" } else { "s" },
        session.name
    );

    let queue = Rc::new(RefCell::new(VecDeque::from(tests)));
    let report = Rc::new(RefCell::new(Report::new(session)));
    let cancel = Rc::new(Cancel::default());

    let grace = Duration::from_secs(u64::from(options.grace));

    let limit = match options.timeout {
        0 => None,
        seconds => Some(Duration::from_secs(u64::from(seconds))),
    };

    let mut workers = (0..usize::from(options.parallel).min(total.max(1)))
        .map(|_| worker(queue.clone(), report.clone(), cancel.clone(), grace, limit))
        .collect::<FuturesUnordered<_>>();

    let mut interrupt = pin!(interrupted()?);

    loop {
        tokio::select! {
            result = workers.next() => match result {
                Some(result) => result?,
                None => break,
            },
            result = &mut interrupt, if !cancel.stopped() => {
                result?;

                println!(
                    "\ninterrupted: finishing what is running, then saving the session \
                     (giving it {}s)",
                    options.grace
                );

                cancel.stop();
            }
        }
    }

    let Report {
        passed,
        failures,
        warnings,
        mut session,
    } = Rc::try_unwrap(report)
        .map_err(|_| anyhow::anyhow!("a worker outlived the run"))?
        .into_inner();

    session.whole = !cancel.stopped();

    let elapsed = started.elapsed();

    if !failures.is_empty() {
        println!("\nfailures:\n");

        for (name, error) in &failures {
            let mut report = String::new();
            _ = write!(report, "{error:?}");

            println!("---- {name} ----");

            for line in report.lines() {
                println!("    {line}");
            }

            println!();
        }

        println!("failures:");

        for (name, _) in &failures {
            println!("    {name}");
        }
    }

    if !warnings.is_empty() {
        println!("\nwarnings:");

        for warning in &warnings {
            println!("    {warning:#}");
        }
    }

    let ok = failures.is_empty() && session.whole;

    let (_, _, missed) = session.counts();

    println!(
        "\ntest result: {}. {passed} passed; {} failed; {missed} not run; finished in {:.2}s",
        if ok { "ok" } else { "FAILED" },
        failures.len(),
        elapsed.as_secs_f64()
    );

    // After the count and before the session's name, so a run read from the
    // bottom answers "did it pass" first and "what is getting slow" second.
    // `--list` returned long before this, so nothing here reaches a listing.
    let slowest = session.slowest(SLOWEST);

    if !slowest.is_empty() {
        let said = slowest
            .iter()
            .map(|(name, seconds)| format!("{name} {seconds:.1}s"))
            .collect::<Vec<_>>()
            .join(", ");

        println!("slowest: {said}");
    }

    match session.save() {
        Ok(path) => println!("session {} written to {}", session.name, path.display()),
        Err(error) => eprintln!("the session could not be written: {error:?}"),
    }

    if !ok {
        match std::env::var("CARGO_PKG_NAME") {
            Ok(package) => {
                println!("run the rest with: cargo test -p {package} -- --last-session")
            }
            Err(_) => println!("run the rest with: {} --last-session", program()),
        }
    }

    Ok(ok)
}

/// Resolves on the first signal that asks this run to stop while leaving what
/// is already running room to finish: Ctrl-C, or SIGTERM, which is how an
/// outer `timeout` and most supervisors end a process (see `run_lock`'s note
/// on the outer `timeout` this suite is usually run under).
///
/// Ctrl-C was already caught this way before SIGTERM joined it here, which is
/// why only SIGTERM is new: both signals default to ending the process at
/// once -- no unwind, no [`Drop`] -- and awaiting [`tokio::signal::ctrl_c`] is
/// what already stood between a Ctrl-C and that. Nothing stood between a
/// SIGTERM and it, which is exactly the gap that left a browser's driver --
/// and the browser under it -- running for days after a run that never got
/// the chance to close them.
#[cfg(unix)]
fn interrupted() -> Result<impl Future<Output = Result<()>>> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut term = signal(SignalKind::terminate()).context("installing a SIGTERM handler")?;
    let mut interrupt = signal(SignalKind::interrupt()).context("installing a SIGINT handler")?;

    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => Ok(()),
            _ = term.recv() => Ok(()),
        }
    })
}

#[cfg(not(unix))]
fn interrupted() -> Result<impl Future<Output = Result<()>>> {
    Ok(async { tokio::signal::ctrl_c().await.context("waiting for Ctrl-C") })
}

/// Set when the run is asked to stop, and what a worker waits on to hear it.
#[derive(Default)]
struct Cancel {
    stopped: Cell<bool>,
    woken: tokio::sync::Notify,
}

impl Cancel {
    fn stopped(&self) -> bool {
        self.stopped.get()
    }

    fn stop(&self) {
        self.stopped.set(true);
        self.woken.notify_waiters();
    }

    /// Resolves once the run has been asked to stop, and at once if it already
    /// has.
    async fn wait(&self) {
        if self.stopped.get() {
            return;
        }

        self.woken.notified().await;
    }
}

/// What the run has seen so far, and the session it is writing down.
struct Report {
    passed: usize,
    failures: Vec<(String, anyhow::Error)>,
    warnings: Vec<anyhow::Error>,
    session: sessions::Session,
}

impl Report {
    fn new(session: sessions::Session) -> Self {
        Self {
            passed: 0,
            failures: Vec::new(),
            warnings: Vec::new(),
            session,
        }
    }

    /// **What one test cost, measured around the test itself.**
    ///
    /// `took` is `None` where nothing ran to time: a browser that would not
    /// open is not a slow test, and recording the time spent failing to start
    /// one as the test's own would read as the test being slow.
    ///
    /// **It is wall clock, and the run is one thread.** `try_run` drives
    /// everything from a `current_thread` runtime, so at `--parallel 2` the two
    /// tests in flight share it and each reading covers whatever its sibling was
    /// doing at the same time. Nothing here divides that out — a reading is what
    /// the test cost in the run that wrote it, which is the honest answer for
    /// "was it slow or was it stuck" and the wrong one for comparing against a
    /// run at a different `--parallel`.
    ///
    /// **How much the sibling costs depends on what it wants, so there is no one
    /// factor to correct by.** A test that spends its time waiting on a browser
    /// barely notices one beside it; a test that spends its time restarting a
    /// server does. Read a reading against others from the same run, not
    /// against a remembered number.
    fn record(&mut self, name: String, result: Result<()>, took: Option<Duration>) {
        let seconds = took.map(|took| took.as_secs_f64());

        match result {
            Ok(()) => {
                self.passed += 1;
                self.session.record(&name, Outcome::Passed, None, seconds);
                println!("test {name} ... ok");
            }
            Err(error) => {
                self.session
                    .record(&name, Outcome::Failed, Some(format!("{error:?}")), seconds);
                println!("test {name} ... FAILED");
                self.failures.push((name, error));
            }
        }
    }

    /// Say something that went wrong around the tests without failing any.
    fn warn(&mut self, warning: anyhow::Error) {
        println!("warning: {warning:#}");
        self.warnings.push(warning);
    }
}

/// Settle what closing a browser answered: a quit that only timed out has
/// already been reaped, so it is said as a warning; anything worse is handed
/// back.
fn settle(report: &RefCell<Report>, closing: Result<Option<anyhow::Error>>) -> Result<()> {
    if let Some(slow) = closing? {
        report.borrow_mut().warn(slow);
    }

    Ok(())
}

/// One browser, and the tests it takes off the queue until there are none or
/// the run is stopped.
///
/// A stop does not cut a test short: it is given `grace` to reach its own end,
/// and what it answers in that time counts. Only a test still running when the
/// grace is up is dropped, and that one is left as it was found - not run,
/// rather than a failure it did not earn.
async fn worker<F: Fixture>(
    queue: Rc<RefCell<VecDeque<Test<F>>>>,
    report: Rc<RefCell<Report>>,
    cancel: Rc<Cancel>,
    grace: Duration,
    limit: Option<Duration>,
) -> Result<()> {
    worker_with(queue, report, cancel, grace, limit, crate::Browser::open).await
}

async fn worker_with<F: Fixture, O: std::future::Future<Output = Result<crate::Browser>>>(
    queue: Rc<RefCell<VecDeque<Test<F>>>>,
    report: Rc<RefCell<Report>>,
    cancel: Rc<Cancel>,
    grace: Duration,
    limit: Option<Duration>,
    mut open_browser: impl FnMut() -> O,
) -> Result<()> {
    let mut browser = None;
    let mut used = 0usize;

    while !cancel.stopped() {
        let Some(Test { name, start, setup }) = queue.borrow_mut().pop_front() else {
            break;
        };

        let mut open = match browser.take() {
            Some(browser) => browser,
            None => {
                used = 0;
                let opening = open_browser();
                let stopped = async {
                    cancel.wait().await;
                    tokio::time::sleep(grace).await;
                };
                match tokio::select! {
                    result = opening => Some(result),
                    () = stopped => None,
                } {
                    Some(Ok(browser)) => browser,
                    Some(Err(error)) => {
                        report.borrow_mut().record(name, Err(error), None);
                        continue;
                    }
                    None => {
                        println!("test {name} ... cut short during browser startup");
                        break;
                    }
                }
            }
        };

        let overrun = async {
            cancel.wait().await;
            tokio::time::sleep(grace).await;
        };

        let waiting = open.driver.waiting();
        let began = Instant::now();

        let running = async {
            let running = crate::run_one(&mut open, start, setup, limit);

            let Some(limit) = limit else {
                return running.await;
            };

            match tokio::time::timeout(limit + CLEANUP_MARGIN, running).await {
                Ok(result) => result,
                Err(_) => Err(anyhow::anyhow!(
                    "{}, and did not stop when it was told to",
                    crate::overran(Some(limit), &waiting)
                )),
            }
        };

        let finished = tokio::select! {
            result = running => Some(result),
            () = overrun => None,
        };

        let Some(mut result) = finished else {
            println!("test {name} ... cut short");

            if let Err(error) = settle(&report, open.close().await) {
                tracing::warn!(?error, "closing the browser after an interrupted test");
            }

            break;
        };

        used += 1;

        if result.is_err() {
            if let Err(error) = settle(&report, open.close().await) {
                tracing::warn!(?error, "closing the browser after a failing test");
                result =
                    result.map_err(|original| crate::driver::with_cleanup(original, Err(error)));
            }
        } else if keeps_browser(used) {
            browser = Some(open);
        } else if let Err(error) = settle(&report, open.close().await) {
            tracing::warn!(?error, "closing a browser that had run its course");
        }

        report
            .borrow_mut()
            .record(name, result, Some(began.elapsed()));
    }

    if let Some(browser) = browser {
        settle(&report, browser.close().await)?;
    }

    Ok(())
}

/// A fixture for the runner's own tests, none of which gets as far as
/// starting one.
#[cfg(test)]
struct Nothing;

#[cfg(test)]
impl Fixture for Nothing {
    type Setup = ();

    async fn start((): ()) -> Result<Self> {
        anyhow::bail!("the runner's own tests start no fixture")
    }

    fn url(&self) -> String {
        String::new()
    }

    async fn quit(self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn a_browser_is_replaced_once_it_has_run_its_course() {
        assert!(keeps_browser(1));
        assert!(keeps_browser(BROWSER_LIFE - 1));
        assert!(!keeps_browser(BROWSER_LIFE));
        assert!(!keeps_browser(BROWSER_LIFE + 1));
    }

    fn queue() -> Rc<RefCell<VecDeque<Test<Nothing>>>> {
        Rc::new(RefCell::new(
            ["first", "second"]
                .into_iter()
                .map(|name| Test {
                    name: name.to_owned(),
                    start: |_, _| Box::pin(async { panic!("a failed browser never runs a test") }),
                    setup: (),
                })
                .collect(),
        ))
    }

    fn report() -> Rc<RefCell<Report>> {
        Rc::new(RefCell::new(Report::new(sessions::Session::new([
            "first".to_owned(),
            "second".to_owned(),
        ]))))
    }

    #[tokio::test]
    async fn startup_failures_are_recorded_and_the_queue_continues() {
        let queue = queue();
        let report = report();
        let attempts = Cell::new(0);
        worker_with(
            queue.clone(),
            report.clone(),
            Rc::new(Cancel::default()),
            Duration::ZERO,
            None,
            || {
                attempts.set(attempts.get() + 1);
                async { Err(anyhow::anyhow!("startup-original-marker")) }
            },
        )
        .await
        .unwrap();
        assert_eq!(attempts.get(), 2);
        assert!(queue.borrow().is_empty());
        let report = report.borrow();
        assert_eq!(report.session.counts(), (0, 2, 0));
        assert_eq!(report.failures[0].0, "first");
        assert_eq!(report.failures[1].0, "second");
        assert!(
            report
                .failures
                .iter()
                .all(|(_, error)| error.to_string().contains("startup-original-marker"))
        );
    }

    #[test]
    fn a_slow_quit_after_passing_tests_is_a_warning() {
        let report = report();
        report.borrow_mut().record("first".to_owned(), Ok(()), None);
        let slow = anyhow::Error::from(crate::driver::TimedOut("quit".to_owned()))
            .context("browser quit: geckodriver");
        settle(&report, Ok(Some(slow))).unwrap();
        let report = report.borrow();
        assert!(report.failures.is_empty());
        assert_eq!(report.passed, 1);
        assert_eq!(report.warnings.len(), 1);
        assert!(format!("{:#}", report.warnings[0]).contains("quit timed out"));
    }

    #[test]
    fn a_slow_quit_does_not_excuse_a_failing_test_or_a_failed_cleanup() {
        let report = report();
        report.borrow_mut().record(
            "first".to_owned(),
            Err(anyhow::anyhow!("test-marker")),
            None,
        );
        let slow = anyhow::Error::from(crate::driver::TimedOut("quit".to_owned()));
        settle(&report, Ok(Some(slow))).unwrap();
        assert_eq!(report.borrow().failures.len(), 1);
        assert_eq!(report.borrow().session.counts(), (0, 1, 1));

        let error = settle(&report, Err(anyhow::anyhow!("reap-marker"))).unwrap_err();
        assert!(error.to_string().contains("reap-marker"));
        assert_eq!(report.borrow().warnings.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_drops_pending_startup_without_failing_unrun_tests() {
        struct Dropped(Rc<Cell<bool>>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let queue = queue();
        let report = report();
        let cancel = Rc::new(Cancel::default());
        let dropped = Rc::new(Cell::new(false));
        worker_with(
            queue.clone(),
            report.clone(),
            cancel.clone(),
            Duration::from_secs(1),
            None,
            || {
                let guard = Dropped(dropped.clone());
                cancel.stop();
                async move {
                    let _guard = guard;
                    std::future::pending().await
                }
            },
        )
        .await
        .unwrap();
        assert!(dropped.get());
        assert_eq!(queue.borrow().len(), 1);
        assert_eq!(report.borrow().session.counts(), (0, 0, 2));
        assert!(report.borrow().failures.is_empty());
    }
}

/// **`interrupted()` catches real SIGTERM and SIGINT, not just the in-process
/// `Cancel` plumbing the tests above exercise.**
///
/// This needs two processes: catching a signal is a whole-process effect, and
/// sending either signal to the thread running this test would take the entire
/// `--lib` binary down with it, every other test included. So each test re-execs
/// itself, filtered down to just itself, with an environment variable telling
/// that second copy to run the child half instead of the real test body; the
/// parent half sends the signal and reads the child's own report of what
/// happened over its stdout.
#[cfg(all(test, unix))]
mod signal_tests {
    use std::io::{BufRead, Write as _};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use tokio::io::AsyncReadExt;

    const CHILD_ENV: &str = "E2E_INTERRUPTED_SIGNAL_CHILD";
    const READY: &str = "armed";
    const CAUGHT: &str = "interrupted";
    const WAIT: Duration = Duration::from_secs(10);

    #[test]
    fn a_sigterm_resolves_interrupted_before_its_future_is_polled() {
        catches(libc::SIGTERM);
    }

    #[test]
    fn a_sigint_resolves_interrupted_before_its_future_is_polled() {
        catches(libc::SIGINT);
    }

    fn catches(signal: i32) {
        if std::env::var_os(CHILD_ENV).is_some() {
            child();
        }

        let exe = std::env::current_exe().expect("this test's own binary");
        let mut child = Command::new(exe)
            .arg(match signal {
                libc::SIGTERM => {
                    "harness::signal_tests::a_sigterm_resolves_interrupted_before_its_future_is_polled"
                }
                libc::SIGINT => {
                    "harness::signal_tests::a_sigint_resolves_interrupted_before_its_future_is_polled"
                }
                _ => unreachable!("only SIGTERM and SIGINT are tested"),
            })
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawning the child copy of this test binary");

        // A guard, not a leaked-process test in its own right: if an
        // assertion below fires first, this still reaps what it started
        // rather than leaving a hung child behind for the next run to trip
        // over -- the exact failure mode this whole card is about.
        struct KillOnDrop(std::process::Child);
        impl Drop for KillOnDrop {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let stdout = child.stdout.take().expect("the child's stdout");
        let mut stdin = child.stdin.take().expect("the child's stdin");
        let mut child = KillOnDrop(child);

        let (lines_tx, lines_rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if lines_tx.send(line).is_err() {
                    break;
                }
            }
        });

        // libtest's own harness talks first -- a blank line, then "running 1
        // test" -- so the marker is somewhere in the stream, not necessarily
        // the next thing in it.
        let recv = |what: &str| {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match lines_rx.recv_timeout(left) {
                    Ok(line) if line == what => return,
                    Ok(_) => continue,
                    Err(_) => panic!("the child never said {what:?} within {WAIT:?}"),
                }
            }
        };

        recv(READY);

        let pid = child.0.id() as i32;
        assert_eq!(
            unsafe { libc::kill(pid, signal) },
            0,
            "sending signal {signal} to the child"
        );

        stdin
            .write_all(b"continue\n")
            .expect("letting the child poll its interrupt future");
        stdin.flush().expect("flushing the child's acknowledgement");

        recv(CAUGHT);

        let status = child.0.wait().expect("waiting for the child to exit");
        assert!(status.success(), "child exited as {status:?}, not cleanly");
    }

    /// The child half, run only inside the re-exec above: install the handlers,
    /// announce readiness, wait for the parent to deliver a signal, then poll
    /// the real `interrupted()` future. That order proves registration happened
    /// before readiness rather than incidentally at the first poll.
    fn child() -> ! {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("building the child's runtime");

        runtime.block_on(async {
            let interrupt =
                super::interrupted().expect("installing handlers before announcing readiness");

            println!("{READY}");
            std::io::stdout().flush().expect("flushing armed");

            let mut acknowledgement = [0];
            tokio::io::stdin()
                .read_exact(&mut acknowledgement)
                .await
                .expect("waiting for the parent acknowledgement");

            interrupt
                .await
                .expect("interrupted() failed instead of catching the signal");

            println!("{CAUGHT}");
            std::io::stdout().flush().expect("flushing interrupted");
        });

        std::process::exit(0)
    }
}

/// How many tests run at once unless the run says otherwise.
const DEFAULT_PARALLEL: u16 = 2;

/// How long a test that is already running is given to finish after a Ctrl-C,
/// before it is dropped. Long enough for a test that is waiting on one last
/// observation, short enough that a hung one does not hold the run open.
const DEFAULT_GRACE: u16 = 30;

/// How many of the slowest tests a finished run names. Enough to show a cluster
/// rather than one outlier, short enough to stay one line.
const SLOWEST: usize = 3;

/// **How long one test may run before the suite stops waiting for it**, and the
/// only cap on a test: raising it with `--timeout` raises the whole of it, and
/// `--timeout 0` really does wait for ever.
///
/// Thirty seconds is room, not a measurement of stuckness. A test waits on
/// observations that each carry a deadline of their own, so most of them cannot
/// approach this - but the longest test in a module is not most of them. A test
/// that runs its whole body twice around a server restart can land just under
/// this on a quiet machine and just over it on a loaded one, and is better
/// split in two than given a longer cap. Read a drop as *how long it took*, and
/// read the wait it names for where the time went.
const DEFAULT_TIMEOUT: u16 = 30;

/// **How much longer than `--timeout` the runner waits before it stops asking
/// nicely.** The cap is applied inside the run, so that a test which overruns
/// is still followed by its session being taken down; this is the room that
/// takedown is given, after which the run drops the test where it stands.
const CLEANUP_MARGIN: Duration = Duration::from_secs(30);

/// How long to wait for a peer runner before giving up.
///
/// **Not long enough to sit out a whole suite, and deliberately so.** The suite
/// is about ten minutes where the `timeout` around the command is usually 600s,
/// so a wait that covered a peer's whole run would leave nothing to run in --
/// and a wait as long as that outer bound is worse than none, because SIGTERM
/// lands at the same moment and takes the runner down mid-wait without a word,
/// which is the failure this cap exists to remove.
///
/// So this is a ceiling on wasted waiting rather than a promise of the lock: it
/// sits out a short peer run, and answers in the runner's own words while the
/// outer budget still has room, instead of queueing until it is killed.
const DEFAULT_LOCK_WAIT: u16 = 300;

/// Where a run's commentary about itself goes. Beside a listing it is stderr,
/// so that what a caller parses out of stdout is only the test names.
fn commentary(listing: bool) -> Box<dyn std::io::Write> {
    if listing {
        Box::new(std::io::stderr())
    } else {
        Box::new(std::io::stdout())
    }
}

/// How many tests one browser takes before it is replaced.
///
/// A browser is reused between tests, and every test navigates it to a fresh
/// port. What that costs is not paid back: the pages behind it stay in session
/// history, each holding its own copy of an unoptimised bundle, and past about
/// thirty of them Firefox stops being able to find room for the next one -
/// `WebAssembly.compile` answers `InternalError: out of memory`, `init` never
/// finishes, and the page mounts nothing at all. Measured, the drops came at 34,
/// 37 and 40 navigations at `--parallel 2`, and at 50 with a single browser,
/// which is the point: halving how many browsers are alive at once did not
/// prevent it, so what accumulates is per browser and not machine-wide. Twenty
/// is comfortably clear of the shallowest of those, and costs one browser start
/// per twenty tests.
const BROWSER_LIFE: usize = 20;

/// Whether a browser that has just carried a test through is kept for the next
/// one, given how many it has carried already.
fn keeps_browser(used: usize) -> bool {
    used < BROWSER_LIFE
}

/// What the run calls itself: the package under test while cargo runs it, and
/// the binary's own name otherwise.
fn program() -> String {
    if let Ok(package) = std::env::var("CARGO_PKG_NAME") {
        return package;
    }

    std::env::args()
        .next()
        .as_deref()
        .map(std::path::Path::new)
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("e2e"))
}

/// What the run was asked for.
#[derive(Parser)]
#[command(name = "e2e", disable_version_flag = true)]
struct Options {
    #[arg(value_name = "FILTER")]
    filters: Vec<String>,

    /// How many tests to run at once.
    #[arg(
        short = 'j',
        long,
        alias = "test-threads",
        default_value_t = DEFAULT_PARALLEL,
        value_parser = clap::value_parser!(u16).range(1..),
    )]
    parallel: u16,

    #[arg(long)]
    headed: bool,

    #[arg(long, value_name = "BROWSER")]
    browser: Option<Engine>,

    #[arg(long)]
    exact: bool,

    #[arg(long)]
    list: bool,

    /// Allow a run that selects no tests.
    #[arg(long)]
    allow_empty: bool,

    /// Run only what did not pass in the most recent session.
    #[arg(long, conflicts_with = "session")]
    last_session: bool,

    /// Run only what did not pass in this session.
    #[arg(long, value_name = "NAME")]
    session: Option<String>,

    /// How long a running test is given to finish after the run is stopped.
    #[arg(long, value_name = "SECONDS", default_value_t = DEFAULT_GRACE)]
    grace: u16,

    /// How long one test may run before it is failed. Zero waits for ever.
    #[arg(long, value_name = "SECONDS", default_value_t = DEFAULT_TIMEOUT)]
    timeout: u16,

    /// How long to wait for another runner holding the same lock to finish.
    /// Zero waits for ever.
    #[arg(long, value_name = "SECONDS", default_value_t = DEFAULT_LOCK_WAIT)]
    lock_wait: u16,

    /// Which other runs this one is held apart from: `target` for the ones
    /// sharing its build directory, `project` for every worktree of this Git
    /// project, `none` for nothing.
    ///
    /// What a run writes -- `e2e-dist`, `e2e-sessions`, the fetched driver --
    /// all lives under the build directory, which `target` is. Ask for `project` to keep two cold `trunk`
    /// builds off the machine at once, or when a wasm-bindgen bump leaves them
    /// racing to fill the shared `~/.cache/trunk` for the first time.
    #[arg(long, value_name = "SCOPE", default_value = "target")]
    lock: crate::run_lock::Scope,

    #[arg(long, hide = true)]
    nocapture: bool,
    #[arg(long, hide = true)]
    show_output: bool,
    #[arg(short, long, hide = true)]
    quiet: bool,
    #[arg(long, hide = true, value_name = "FORMAT")]
    format: Option<String>,
}

impl Options {
    fn wants(&self, name: &str) -> bool {
        if self.filters.is_empty() {
            return true;
        }

        self.filters.iter().any(|filter| {
            if self.exact {
                name == filter
            } else {
                name.contains(filter)
            }
        })
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn empty_selection_requires_opt_in_except_when_listing() {
        for args in [
            vec!["e2e", "missing"],
            vec!["e2e", "present", "--exact"],
            vec!["e2e", "missing", "--allow-empty"],
            vec!["e2e", "missing", "--list"],
            vec!["e2e", "present", "--list"],
        ] {
            let options = Options::parse_from(&args);
            let allowed = options.allow_empty || options.list;
            let mut suite = Suite::<Nothing>::new();
            suite.add(
                "module::present",
                |_, _| Box::pin(async { panic!("selection checks must not run a browser test") }),
                (),
            );

            let result = suite.try_run(options);
            if allowed {
                assert!(result.unwrap(), "{args:?}");
            } else {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    "no tests selected by the filter — this is not a pass; the executable may be stale",
                    "{args:?}"
                );
            }
        }
    }
}

/// Declare the suite and the entrypoint that runs it.
///
/// The [`Fixture`](crate::Fixture) every test is handed comes first, then one
/// list, in one file, which is the whole of what says a test exists: an
/// undeclared test function is dead code and the compiler says so. A test
/// needing something other than the bare fixture names it in parentheses, each
/// word being a `bool` field of the fixture's `Setup` that is set for it.
///
/// ```ignore
/// yew_e2e::harness! {
///     App;
///     cart::{adds_an_item, removes_an_item},
///     login::{remembers_the_user(signed_in)},
/// }
/// ```
#[macro_export]
macro_rules! harness {
    (
        $fixture:ty;
        $($module:ident :: {
            $($test:ident $(($($setup:ident),* $(,)?))?),* $(,)?
        }),* $(,)?
    ) => {
        fn main() -> ! {
            let mut suite = $crate::harness::Suite::<$fixture>::new();

            $(
                $(
                    suite.add(
                        concat!(stringify!($module), "::", stringify!($test)),
                        {
                            fn start<'a>(
                                driver: &'a mut $crate::TestDriver,
                                fixture: &'a mut $fixture,
                            ) -> $crate::harness::TestFuture<'a> {
                                Box::pin($module::$test(driver, fixture))
                            }

                            start
                        },
                        {
                            #[allow(unused_mut)]
                            let mut setup = <<$fixture as $crate::Fixture>::Setup as ::core::default::Default>::default();
                            $($(setup.$setup = true;)*)?
                            setup
                        },
                    );
                )*
            )*

            suite.run()
        }
    };
}
