//! What one run of the suite did, kept so the next run can pick up where it
//! left off.
//!
//! A *session* here is a record of a run, and is not a [`crate::Fixture`],
//! which is the application started for one test. Each run writes one, named after the
//! moment it started so the newest sorts last, and `--last-session` reads the
//! newest back and runs everything in it that did not pass.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use musli::{Decode, Encode};

/// Bumped only if the shape changes incompatibly. An unrecognized version reads
/// as nothing, which costs a session and not a run.
const VERSION: u32 = 1;

/// How one test ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[musli(Text, name_all = "kebab-case")]
pub(crate) enum Outcome {
    Passed,
    Failed,
    /// Chosen by the run and never reached, which is what an interrupted run
    /// leaves behind.
    NotRun,
}

/// One test, and how it ended.
#[derive(Debug, Clone, Encode, Decode)]
#[musli(Text, name_all = "kebab-case")]
pub(crate) struct Ran {
    pub(crate) name: String,
    pub(crate) outcome: Outcome,
    /// What it failed with, kept because the run that saw it is gone by the
    /// time anyone reads the session back. Absent for anything that did not
    /// fail, and absent from a session written before this was recorded.
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub(crate) message: Option<String>,
    /// **How long it took, in seconds**, which is what says whether a test that
    /// was cut off was stuck or only slow.
    ///
    /// **Wall clock, and the whole run is one thread.** The suite drives its
    /// browsers from a `current_thread` runtime, so at `--parallel 2` two tests
    /// share it and each one's reading covers whatever its sibling was doing at
    /// the same time. A reading is therefore what that test cost *in that run*,
    /// not what it would cost alone: read two runs against each other only when
    /// they were run at the same `--parallel`.
    ///
    /// Absent for a test that never ran, and absent from a session written
    /// before this was recorded.
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub(crate) seconds: Option<f64>,
}

/// One run of the suite.
#[derive(Debug, Clone, Encode, Decode)]
#[musli(Text, name_all = "kebab-case")]
pub(crate) struct Session {
    version: u32,
    pub(crate) name: String,
    /// Seconds since the epoch, which is also what the name leads with.
    pub(crate) started: u64,
    /// Whether the run reached the end of its queue rather than being
    /// interrupted.
    pub(crate) whole: bool,
    pub(crate) tests: Vec<Ran>,
}

impl Session {
    /// A session of everything this run chose, none of it run yet.
    pub(crate) fn new(names: impl IntoIterator<Item = String>) -> Self {
        let started = now();

        Self {
            version: VERSION,
            name: name_for(started),
            started,
            whole: false,
            tests: names
                .into_iter()
                .map(|name| Ran {
                    name,
                    outcome: Outcome::NotRun,
                    message: None,
                    seconds: None,
                })
                .collect(),
        }
    }

    /// Say how one test ended, what it said if it failed, and how long it took.
    ///
    /// `seconds` is `None` where nothing ran to be timed -- a browser that
    /// would not open is not a slow test.
    pub(crate) fn record(
        &mut self,
        name: &str,
        outcome: Outcome,
        message: Option<String>,
        seconds: Option<f64>,
    ) {
        if let Some(ran) = self.tests.iter_mut().find(|ran| ran.name == name) {
            ran.outcome = outcome;
            ran.message = message;
            ran.seconds = seconds;
        }
    }

    /// Every test that failed and said why, which is what a run picking the
    /// session up can show of the run that wrote it.
    pub(crate) fn failures(&self) -> impl Iterator<Item = (&str, &str, Option<f64>)> {
        self.tests.iter().filter_map(|ran| {
            let message = ran.message.as_deref()?;
            (ran.outcome == Outcome::Failed).then_some((ran.name.as_str(), message, ran.seconds))
        })
    }

    /// **The few that cost the most, longest first**, which is what says a test
    /// is drifting towards its cap while it is still passing. A run only ever
    /// reported the ones that had already gone over.
    ///
    /// Anything without a reading is left out rather than sorted as a zero: a
    /// test that never ran did not take no time, it took no time *because it
    /// did not run*, and sorting it against the ones that did would put it at
    /// the wrong end of the list.
    pub(crate) fn slowest(&self, most: usize) -> Vec<(&str, f64)> {
        let mut timed = self
            .tests
            .iter()
            .filter_map(|ran| Some((ran.name.as_str(), ran.seconds?)))
            .collect::<Vec<_>>();

        timed.sort_by(|a, b| b.1.total_cmp(&a.1));
        timed.truncate(most);
        timed
    }

    /// Every test this session did not see pass, which is what a run picking it
    /// up should do: the ones that failed, and the ones it never reached.
    pub(crate) fn unfinished(&self) -> Vec<&str> {
        self.tests
            .iter()
            .filter(|ran| ran.outcome != Outcome::Passed)
            .map(|ran| ran.name.as_str())
            .collect()
    }

    /// How many ended each way.
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        let mut counts = (0, 0, 0);

        for ran in &self.tests {
            match ran.outcome {
                Outcome::Passed => counts.0 += 1,
                Outcome::Failed => counts.1 += 1,
                Outcome::NotRun => counts.2 += 1,
            }
        }

        counts
    }

    /// Write it down, replacing whatever was there under the same name. Written
    /// beside itself and renamed, so an interrupted write leaves the last whole
    /// session standing rather than half of this one.
    pub(crate) fn save(&self) -> Result<PathBuf> {
        let dir = dir()?;
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

        let path = dir.join(format!("{}.json", self.name));
        let temporary = path.with_extension("json.writing");

        let text = musli::json::to_string(self).context("writing the session")?;

        fs::write(&temporary, text).with_context(|| format!("writing {}", temporary.display()))?;
        fs::rename(&temporary, &path).with_context(|| format!("naming {}", path.display()))?;

        Ok(path)
    }
}

/// Where sessions are kept, which is with the rest of what the suite leaves
/// behind.
pub(crate) fn dir() -> Result<PathBuf> {
    let name = DIR.get().map_or("e2e-sessions", String::as_str);
    Ok(crate::dist::target_dir()?.join(name))
}

/// What [`dir`] is called, once the suite has said; see
/// [`crate::Config::sessions_dir`].
static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Name the directory sessions are kept in, before any is read or written.
pub(crate) fn name_dir(name: &str) {
    _ = DIR.set(name.to_owned());
}

/// Read one back by name.
pub(crate) fn load(name: &str) -> Result<Session> {
    let path = dir()?.join(format!("{name}.json"));

    let text = fs::read(&path)
        .with_context(|| format!("no session called {name} at {}", path.display()))?;

    let session: Session =
        musli::json::from_slice(&text).with_context(|| format!("reading {}", path.display()))?;

    anyhow::ensure!(
        session.version == VERSION,
        "{name} was written by another build of the suite"
    );

    Ok(session)
}

/// The most recent session, by the time its run started.
pub(crate) fn latest() -> Result<Session> {
    let dir = dir()?;

    let Ok(entries) = fs::read_dir(&dir) else {
        anyhow::bail!(
            "no sessions yet in {} - run the suite once first",
            dir.display()
        );
    };

    let mut newest: Option<Session> = None;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }

        let Ok(text) = fs::read(&path) else {
            continue;
        };

        let Ok(session) = musli::json::from_slice::<Session>(&text) else {
            continue;
        };

        if session.version != VERSION {
            continue;
        }

        if newest
            .as_ref()
            .is_none_or(|held| session.started > held.started)
        {
            newest = Some(session);
        }
    }

    newest.with_context(|| format!("no session in {} this build can read", dir.display()))
}

/// Seconds since the epoch, or zero on a clock that is before it.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// A name that sorts by when the run started and is still sayable: the day and
/// the time, two words, and enough randomness that two runs in one second do
/// not collide.
fn name_for(started: u64) -> String {
    let (date, time) = stamp(started);

    let adjective = ADJECTIVES[rand::random_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rand::random_range(0..NOUNS.len())];
    let tail = rand::random::<u16>();

    format!("{date}-{time}-{adjective}-{noun}-{tail:04x}")
}

/// The day and the time of one instant, as `yyyymmdd` and `hhmmss` in UTC.
///
/// Done here rather than with a calendar crate: the suite wants a name that
/// sorts, which is a division and not a date library.
fn stamp(seconds: u64) -> (String, String) {
    let days = seconds / 86_400;
    let rest = seconds % 86_400;

    let (year, month, day) = civil(days);

    (
        format!("{year:04}{month:02}{day:02}"),
        format!(
            "{:02}{:02}{:02}",
            rest / 3_600,
            (rest % 3_600) / 60,
            rest % 60
        ),
    )
}

/// Days since 1970-01-01 as a civil date, by Howard Hinnant's algorithm.
fn civil(days: u64) -> (u64, u64, u64) {
    let days = days + 719_468;

    let era = days / 146_097;
    let of_era = days % 146_097;

    let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;

    let year = year_of_era + era * 400;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);

    let month_of_year = (5 * of_year + 2) / 153;

    let day = of_year - (153 * month_of_year + 2) / 5 + 1;
    let month = if month_of_year < 10 {
        month_of_year + 3
    } else {
        month_of_year - 9
    };

    (year + u64::from(month <= 2), month, day)
}

const ADJECTIVES: [&str; 32] = [
    "amber", "brisk", "calm", "clear", "cool", "deep", "dry", "even", "fair", "firm", "flat",
    "fresh", "glad", "keen", "level", "light", "loud", "mild", "near", "open", "plain", "quick",
    "quiet", "round", "sharp", "short", "slow", "soft", "steady", "still", "warm", "wide",
];

const NOUNS: [&str; 32] = [
    "amp", "band", "bay", "bell", "bus", "cable", "chord", "cue", "dial", "drum", "fader", "gate",
    "horn", "jack", "knob", "lane", "meter", "mic", "note", "patch", "pedal", "pitch", "port",
    "reed", "reel", "rig", "room", "stage", "strip", "take", "tone", "track",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A name leads with the day and the time so a listing is in the order the
    /// runs happened.
    #[test]
    fn a_name_sorts_by_when_the_run_started() {
        let first = name_for(1_756_900_000);
        let second = name_for(1_756_900_001);

        assert!(first < second, "{first} did not sort before {second}");
    }

    /// The date arithmetic, against days a calendar disagrees about.
    #[test]
    fn a_day_count_reads_as_a_date() {
        assert_eq!(stamp(0).0, "19700101");
        assert_eq!(stamp(86_399).1, "235959");
        assert_eq!(stamp(951_782_400).0, "20000229", "a leap day");
        assert_eq!(stamp(1_756_900_000).0, "20250903");
    }

    /// What a run picks up is everything the last one did not see pass, which is
    /// the failures and whatever it never reached.
    #[test]
    fn a_session_hands_on_what_did_not_pass() {
        let mut session = Session::new(["a", "b", "c", "d"].map(String::from));

        session.record("a", Outcome::Passed, None, Some(1.0));
        session.record(
            "b",
            Outcome::Failed,
            Some("b said no".to_owned()),
            Some(2.5),
        );
        session.record("c", Outcome::Passed, None, Some(0.5));

        assert_eq!(session.unfinished(), ["b", "d"]);
        assert_eq!(session.counts(), (2, 1, 1));
        assert_eq!(
            session.failures().collect::<Vec<_>>(),
            [("b", "b said no", Some(2.5))]
        );
    }

    /// A session survives being written down and read back, which is the only
    /// thing standing between two runs.
    #[test]
    fn a_session_survives_the_round_trip() {
        let mut session = Session::new(["one", "two"].map(String::from));
        session.record(
            "one",
            Outcome::Failed,
            Some("one said no".to_owned()),
            Some(12.5),
        );
        session.whole = true;

        let text = musli::json::to_string(&session).expect("a session encodes");
        let back: Session = musli::json::from_slice(text.as_bytes()).expect("and decodes");

        assert_eq!(back.name, session.name);
        assert_eq!(back.started, session.started);
        assert!(back.whole);
        assert_eq!(back.unfinished(), ["one", "two"]);

        assert_eq!(
            back.failures().collect::<Vec<_>>(),
            [("one", "one said no", Some(12.5))]
        );

        // The test that never ran carries no reading, and says so rather than
        // saying zero: a test that was not reached did not take no time.
        let two = back.tests.iter().find(|ran| ran.name == "two").unwrap();
        assert_eq!(two.seconds, None);
    }

    /// **The slowest few are the timed ones, longest first**, and a test with no
    /// reading is left out rather than sorted to the fast end. The point of the
    /// line is to name what is drifting towards the cap while it still passes,
    /// so an untimed test appearing in it would be noise in the one place there
    /// is no room for any.
    #[test]
    fn the_slowest_are_the_timed_ones_longest_first() {
        let mut session =
            Session::new(["quick", "slow", "middling", "never-ran"].map(String::from));

        session.record("quick", Outcome::Passed, None, Some(1.5));
        session.record("slow", Outcome::Passed, None, Some(30.25));
        session.record(
            "middling",
            Outcome::Failed,
            Some("no".to_owned()),
            Some(9.0),
        );

        assert_eq!(
            session.slowest(3),
            [("slow", 30.25), ("middling", 9.0), ("quick", 1.5)]
        );

        // Fewer than asked for is what there is, and the cap is a cap.
        assert_eq!(session.slowest(2), [("slow", 30.25), ("middling", 9.0)]);
        assert_eq!(session.slowest(10).len(), 3);

        // Nothing timed at all says nothing rather than naming everything at
        // zero, which is what keeps the line off a run that was interrupted
        // before it timed anything.
        let untimed = Session::new(["one".to_owned()]);
        assert!(untimed.slowest(3).is_empty());
    }

    /// A session written before a failure carried its message still reads,
    /// since picking one up matters more than the message it does not have.
    #[test]
    fn a_session_without_messages_still_reads() {
        let text = format!(
            concat!(
                r#"{{"version":{version},"name":"n","started":1,"whole":true,"#,
                r#""tests":[{{"name":"one","outcome":"failed"}}]}}"#
            ),
            version = VERSION
        );

        let session: Session = musli::json::from_slice(text.as_bytes()).expect("a session decodes");

        assert_eq!(session.unfinished(), ["one"]);
        assert_eq!(session.failures().count(), 0);
    }

    /// **A session written before durations were recorded still reads**, and
    /// says it has none rather than claiming the tests took no time. This is
    /// what keeps `VERSION` where it is: the field is skipped when absent, so
    /// the shape did not break and an older session is still worth picking up.
    #[test]
    fn a_session_without_durations_still_reads() {
        let text = format!(
            concat!(
                r#"{{"version":{version},"name":"n","started":1,"whole":true,"#,
                r#""tests":[{{"name":"one","outcome":"failed","message":"one said no"}},"#,
                r#"{{"name":"two","outcome":"passed"}}]}}"#
            ),
            version = VERSION
        );

        let session: Session = musli::json::from_slice(text.as_bytes()).expect("a session decodes");

        assert_eq!(session.unfinished(), ["one"]);
        assert_eq!(
            session.failures().collect::<Vec<_>>(),
            [("one", "one said no", None)],
            "an older session reported a duration it never carried"
        );
    }

    /// **A test that passed carries its reading too**, not only the ones that
    /// failed: a pass that took the whole cap is the one worth finding before
    /// it becomes a failure.
    #[test]
    fn a_passing_test_is_timed_as_well() {
        let mut session = Session::new(["one"].map(String::from));
        session.record("one", Outcome::Passed, None, Some(3.25));

        let text = musli::json::to_string(&session).expect("a session encodes");
        let back: Session = musli::json::from_slice(text.as_bytes()).expect("and decodes");

        assert_eq!(back.tests[0].seconds, Some(3.25));
    }
}
