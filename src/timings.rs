//! **Where one test's time went**, for `--timings`: how long each phase of the
//! test took, and the waits that took longest.
//!
//! Nothing here is measured unless the runner scoped a [`Timings`] around the
//! test with [`within`]; outside one, every function here answers at once and
//! nothing is formatted, so a run without `--timings` reads and costs as it
//! always has.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::future::Future;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How many of a test's waits its report names.
const SLOWEST: usize = 5;

/// How long a wait must have taken to be named at all.
const SLOW_WAIT: Duration = Duration::from_millis(100);

tokio::task_local! {
    /// What the test under way has measured so far, while `--timings` is on.
    static TIMINGS: Rc<RefCell<Timings>>;
}

/// One part of running a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Starting the WebDriver server, on the test that opened a browser.
    DriverSpawn,
    /// Creating the WebDriver session, on the test that opened a browser.
    SessionCreate,
    /// [`Fixture::start`](crate::Fixture::start).
    FixtureStart,
    /// Swapping in a fresh tab and sizing the window.
    TabReset,
    /// Navigating to the fixture's page.
    Goto,
    /// The test itself.
    Body,
    /// [`Fixture::quit`](crate::Fixture::quit).
    FixtureQuit,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Phase::DriverSpawn => "driver",
            Phase::SessionCreate => "session",
            Phase::FixtureStart => "fixture start",
            Phase::TabReset => "reset",
            Phase::Goto => "goto",
            Phase::Body => "body",
            Phase::FixtureQuit => "fixture quit",
        }
    }
}

/// One wait a test made: how long it took, how many times it looked, and what
/// it was waiting for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Wait {
    pub(crate) took: Duration,
    pub(crate) attempts: usize,
    pub(crate) what: String,
}

/// What one test measured.
#[derive(Default, Debug)]
pub(crate) struct Timings {
    pub(crate) phases: Vec<(Phase, Duration)>,
    pub(crate) waits: Vec<Wait>,
}

/// Run `future` with `timings` collecting what it measures, or just run it
/// where there is nothing to collect into.
pub(crate) async fn within<T>(
    timings: Option<&Rc<RefCell<Timings>>>,
    future: impl Future<Output = T>,
) -> T {
    match timings {
        Some(timings) => TIMINGS.scope(timings.clone(), future).await,
        None => future.await,
    }
}

/// Whether anything is collecting what this task measures.
pub(crate) fn active() -> bool {
    TIMINGS.try_with(|_| ()).is_ok()
}

/// Run `future`, counting the time it took as `phase` whether or not it
/// succeeded.
pub(crate) async fn phase<T>(phase: Phase, future: impl Future<Output = T>) -> T {
    if !active() {
        return future.await;
    }

    let began = Instant::now();
    let output = future.await;
    let took = began.elapsed();

    _ = TIMINGS.try_with(|timings| timings.borrow_mut().phases.push((phase, took)));

    output
}

/// Count a wait that took `took` over `attempts` looks. `what` is the words it
/// would have given up with; one that answered did not time out, so the
/// `Timed out after ...` they start with is left off.
pub(crate) fn wait(took: Duration, attempts: usize, answered: bool, what: String) {
    let what = match answered {
        true => untimed(&what).to_owned(),
        false => what,
    };

    _ = TIMINGS.try_with(|timings| {
        timings.borrow_mut().waits.push(Wait {
            took,
            attempts,
            what,
        });
    });
}

/// `what` without a leading `Timed out after <duration> `.
fn untimed(what: &str) -> &str {
    what.strip_prefix("Timed out after ")
        .and_then(|rest| rest.split_once(' '))
        .map_or(what, |(_, rest)| rest)
}

/// The lines a test's report gains under `--timings`, each indented and ending
/// in a newline: its phases on one line, then its slowest waits, slowest first.
pub(crate) fn render(timings: &Timings) -> String {
    let mut out = String::new();

    if !timings.phases.is_empty() {
        out.push_str("    timings:");

        for (index, (phase, took)) in timings.phases.iter().enumerate() {
            let sep = if index == 0 { " " } else { ", " };
            _ = write!(out, "{sep}{} {}", phase.label(), human(*took));
        }

        out.push('\n');
    }

    let mut waits = timings
        .waits
        .iter()
        .filter(|wait| wait.took >= SLOW_WAIT)
        .collect::<Vec<_>>();
    waits.sort_by_key(|wait| std::cmp::Reverse(wait.took));

    for wait in waits.into_iter().take(SLOWEST) {
        let looks = if wait.attempts == 1 {
            "attempt"
        } else {
            "attempts"
        };

        _ = writeln!(
            out,
            "    wait {} ({} {looks}) {}",
            human(wait.took),
            wait.attempts,
            wait.what
        );
    }

    out
}

/// A duration as a person reads one: milliseconds under a second, else
/// seconds to two places.
fn human(took: Duration) -> String {
    if took < Duration::from_secs(1) {
        format!("{}ms", took.as_millis())
    } else {
        format!("{:.2}s", took.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use super::{Phase, Timings, Wait, active, phase, render, untimed, wait, within};

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    fn slow(took: u64, what: &str) -> Wait {
        Wait {
            took: ms(took),
            attempts: 3,
            what: what.to_owned(),
        }
    }

    #[test]
    fn an_answered_wait_drops_the_timeout_it_did_not_reach() {
        assert_eq!(
            untimed("Timed out after 5s waiting for #a"),
            "waiting for #a"
        );
        assert_eq!(untimed("Timed out after 1.5s acting on #b"), "acting on #b");
        assert_eq!(untimed("the poll timed out"), "the poll timed out");
    }

    #[test]
    fn a_passing_tests_phases_read_on_one_line() {
        let timings = Timings {
            phases: vec![
                (Phase::DriverSpawn, ms(8)),
                (Phase::SessionCreate, ms(1020)),
                (Phase::FixtureStart, ms(20)),
                (Phase::TabReset, ms(330)),
                (Phase::Goto, ms(25)),
                (Phase::Body, ms(2104)),
                (Phase::FixtureQuit, ms(0)),
            ],
            waits: Vec::new(),
        };

        assert_eq!(
            render(&timings),
            "    timings: driver 8ms, session 1.02s, fixture start 20ms, reset 330ms, \
             goto 25ms, body 2.10s, fixture quit 0ms\n"
        );
    }

    #[test]
    fn a_failed_test_names_only_the_phases_that_ran() {
        let timings = Timings {
            phases: vec![(Phase::FixtureStart, ms(20)), (Phase::TabReset, ms(31))],
            waits: Vec::new(),
        };

        assert_eq!(
            render(&timings),
            "    timings: fixture start 20ms, reset 31ms\n"
        );
    }

    #[test]
    fn nothing_measured_says_nothing() {
        assert_eq!(render(&Timings::default()), "");
    }

    #[test]
    fn the_slowest_waits_come_first_and_quick_ones_are_left_out() {
        let mut waits = (0..7)
            .map(|n| slow(100 + n * 100, &format!("waiting for #{n}")))
            .collect::<Vec<_>>();
        waits.push(slow(99, "waiting for #quick"));
        waits.push(Wait {
            took: ms(150),
            attempts: 1,
            what: "acting on #once".to_owned(),
        });

        let timings = Timings {
            phases: vec![(Phase::Body, ms(4000))],
            waits,
        };

        assert_eq!(
            render(&timings),
            "    timings: body 4.00s\n\
             \x20   wait 700ms (3 attempts) waiting for #6\n\
             \x20   wait 600ms (3 attempts) waiting for #5\n\
             \x20   wait 500ms (3 attempts) waiting for #4\n\
             \x20   wait 400ms (3 attempts) waiting for #3\n\
             \x20   wait 300ms (3 attempts) waiting for #2\n"
        );

        let timings = Timings {
            phases: Vec::new(),
            waits: vec![
                slow(99, "waiting for #quick"),
                Wait {
                    took: ms(150),
                    attempts: 1,
                    what: "acting on #once".to_owned(),
                },
            ],
        };

        assert_eq!(
            render(&timings),
            "    wait 150ms (1 attempt) acting on #once\n"
        );
    }

    #[tokio::test]
    async fn nothing_is_collected_outside_a_scope() {
        assert!(!active());
        phase(Phase::Body, async {}).await;
        wait(ms(500), 2, true, "waiting for #a".to_owned());

        let timings = Rc::new(RefCell::new(Timings::default()));
        within(None, async {
            assert!(!active());
            phase(Phase::Body, async {}).await;
        })
        .await;
        assert!(timings.borrow().phases.is_empty());
    }

    #[tokio::test]
    async fn phases_and_waits_are_collected_inside_a_scope() {
        let timings = Rc::new(RefCell::new(Timings::default()));

        within(Some(&timings), async {
            assert!(active());
            phase(Phase::FixtureStart, async {}).await;
            wait(
                ms(500),
                2,
                true,
                "Timed out after 5s waiting for #a".to_owned(),
            );
            wait(
                ms(5000),
                9,
                false,
                "Timed out after 5s waiting for #b".to_owned(),
            );
        })
        .await;

        let timings = timings.borrow();
        assert_eq!(timings.phases.len(), 1);
        assert_eq!(
            timings.phases.first().map(|(p, _)| *p),
            Some(Phase::FixtureStart)
        );
        assert_eq!(
            timings.waits,
            [
                Wait {
                    took: ms(500),
                    attempts: 2,
                    what: "waiting for #a".to_owned()
                },
                Wait {
                    took: ms(5000),
                    attempts: 9,
                    what: "Timed out after 5s waiting for #b".to_owned()
                },
            ]
        );
    }
}
