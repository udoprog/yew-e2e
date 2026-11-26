//! What every lookup and every wait shares: how often a wait looks again, how
//! long it looks for, the methods [`TestDriver`] and [`TestElement`] both
//! have, and [`IntoBy`], which names what they look for.
//!
//! [`TestDriver`]: crate::TestDriver
//! [`TestElement`]: crate::TestElement

use std::fmt;

use anyhow::Result;
use thirtyfour::prelude::*;
use tokio::time::{self, Duration, Instant};

/// How often a pending observation is repeated.
pub const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// How long any one observation of a page that is already up may take to agree.
pub(crate) const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the first look at a page may take: the browser starting, the bundle
/// being fetched, the wasm compiled and the application connected, none of
/// which has happened when a test first asks for the page; see
/// [`TestDriver::load_timeout`].
pub(crate) const LOAD_TIMEOUT: Duration = Duration::from_secs(15);

tokio::task_local! {
    /// When the innermost wait under way gives up. Every lookup made while
    /// one is under way is bounded by it, so that no one slow find can carry
    /// a wait past its own deadline.
    static DEADLINE: Instant;
}

/// **A wait or a lookup that ran out of time**, as opposed to one that
/// failed. A wait whose attempt ran out of the wait's own time gives up with
/// its own words rather than passing on the attempt's.
#[derive(Debug)]
pub(crate) struct OutOfTime(pub(crate) String);

impl fmt::Display for OutOfTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OutOfTime {}

/// `timeout`, cut to what is left of the wait under way, if one is.
pub(crate) fn bounded(timeout: Duration) -> Duration {
    DEADLINE
        .try_with(|deadline| timeout.min(deadline.saturating_duration_since(Instant::now())))
        .unwrap_or(timeout)
}

/// Whether `error` is, or was caused by, WebDriver saying an element handle is
/// stale: the element it named was taken out of the page, as a redraw does
/// between a lookup and a read of what it found.
pub(crate) fn is_stale(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<thirtyfour::error::WebDriverError>()
            .is_some_and(|error| {
                matches!(
                    error.as_inner(),
                    thirtyfour::error::WebDriverErrorInner::StaleElementReference(..)
                )
            })
    })
}

/// **Repeat `attempt` until it answers, or `timeout` passes**, looking again
/// every [`POLL_INTERVAL`]. Running out of time is an [`OutOfTime`] saying
/// what `waited` says was being waited for.
///
/// A poll inside a wait gives up no later than that wait does, and every
/// lookup an attempt makes is bounded by what is left of the poll's deadline;
/// an attempt that failed by running out of that time ran out of time, and is
/// not a failure of its own.
///
/// **An attempt that met a stale element has not answered yet**, rather than
/// failed: the page redrew between the attempt finding an element and reading
/// it, and the next attempt finds it afresh. Should the poll run out of time,
/// the last such error is attached to the timeout, since it may be why.
pub(crate) async fn poll<T>(
    timeout: Duration,
    waited: impl FnOnce() -> String,
    mut attempt: impl AsyncFnMut() -> Result<Option<T>>,
) -> Result<T> {
    let own = Instant::now() + timeout;
    let deadline = DEADLINE.try_with(|outer| own.min(*outer)).unwrap_or(own);

    let mut stale = None;

    let found = DEADLINE
        .scope(deadline, async {
            loop {
                match attempt().await {
                    Ok(Some(found)) => return Ok(Some(found)),
                    Ok(None) => {}
                    Err(error)
                        if error.downcast_ref::<OutOfTime>().is_some()
                            && Instant::now() >= deadline => {}
                    Err(error) if is_stale(&error) => {
                        stale = Some(error);
                    }
                    Err(error) => return Err(error),
                }

                if Instant::now() >= deadline {
                    return Ok(None);
                }

                time::sleep(POLL_INTERVAL).await;
            }
        })
        .await?;

    match found {
        Some(found) => Ok(found),
        None => {
            let error = anyhow::Error::from(OutOfTime(waited()));

            Err(match stale {
                Some(stale) => {
                    error.context(format!("last attempt met a stale element: {stale:#}"))
                }
                None => error,
            })
        }
    }
}

macro_rules! common {
    () => {
        /// Repeat `condition` until it holds, or the wait timeout passes.
        pub async fn wait_until(
            &self,
            what: impl std::fmt::Display,
            condition: impl AsyncFnMut() -> Result<bool>,
        ) -> Result<()> {
            self.wait_for(self.wait_timeout, what, condition).await
        }

        /// [`Self::wait_until`] where the default deadline is the wrong one,
        /// because what is being waited for has a length of its own that is
        /// known and is close to it. A wait that habitually lands just inside
        /// its cap is not passing, it is winning a race.
        ///
        /// Every lookup `condition` makes is bounded by what is left of
        /// `timeout`, so a slow one cannot carry the wait past it.
        pub async fn wait_until_within(
            &self,
            timeout: Duration,
            what: impl std::fmt::Display,
            condition: impl AsyncFnMut() -> Result<bool>,
        ) -> Result<()> {
            self.wait_for(timeout, what, condition).await
        }

        /// [`Self::wait_until`] against a deadline of the caller's choosing.
        async fn wait_for(
            &self,
            timeout: Duration,
            what: impl std::fmt::Display,
            mut condition: impl AsyncFnMut() -> Result<bool>,
        ) -> Result<()> {
            self.waiting.on(&what);

            crate::finding::poll(
                timeout,
                || format!("Timed out after {timeout:?} waiting for {what}"),
                async || Ok(condition().await?.then_some(())),
            )
            .await
        }

        /// Wait until the text of every node matching `by` reads `expected`.
        pub async fn wait_texts<const N: usize>(
            &self,
            by: impl IntoBy,
            expected: [&str; N],
        ) -> Result<()> {
            let by = by.into_by();

            tracing::trace!(?by, ?expected, "Waiting for texts");

            let seen = std::cell::RefCell::new(Vec::new());

            let result = self
                .wait_until(format_args!("{by} to read {expected:?}"), async || {
                    let texts = self.find_all_texts(by.clone()).await?;
                    let matched = texts == expected;
                    *seen.borrow_mut() = texts;
                    Ok(matched)
                })
                .await;

            match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    let seen = seen.into_inner();
                    Err(error.context(format!("last read {seen:?}")))
                }
            }
        }

        /// Wait until the one node matching `by` reads exactly `expected`,
        /// found afresh on every look, so a redraw that replaces it is
        /// followed rather than failed on.
        ///
        /// Giving up says what it last read. Like [`Self::find_one_by`], a
        /// second match is refused rather than picked from; for several, see
        /// [`Self::wait_texts`].
        pub async fn wait_text(&self, by: impl IntoBy, expected: &str) -> Result<()> {
            let by = by.into_by();

            tracing::trace!(?by, expected, "Waiting for text");

            self.wait_one_text(
                &by,
                format_args!("{by} to read {expected:?}"),
                |text| text == expected,
            )
            .await
        }

        /// The same as [`Self::wait_text`], for the one node matching `by` to
        /// read something with `part` in it.
        pub async fn wait_text_contains(&self, by: impl IntoBy, part: &str) -> Result<()> {
            let by = by.into_by();

            tracing::trace!(?by, part, "Waiting for text containing");

            self.wait_one_text(
                &by,
                format_args!("{by} to contain {part:?}"),
                |text| text.contains(part),
            )
            .await
        }

        /// Wait for the one node matching `by` to read something `matches`
        /// agrees with, saying what was last read when giving up.
        async fn wait_one_text(
            &self,
            by: &By,
            what: impl std::fmt::Display,
            matches: impl Fn(&str) -> bool,
        ) -> Result<()> {
            let seen = std::cell::RefCell::new(None::<String>);

            let result = self
                .wait_until(what, async || {
                    let items = self.find_all(by.clone()).await?;

                    ensure!(
                        items.len() <= 1,
                        "Expected exactly one by {by} to read, but got {}: where \
                         several is what the page is, wait for all of their \
                         texts with wait_texts",
                        items.len()
                    );

                    let Some(item) = items.first() else {
                        return Ok(false);
                    };

                    let text = item.text().await?;
                    let matched = matches(&text);
                    *seen.borrow_mut() = Some(text);
                    Ok(matched)
                })
                .await;

            result.map_err(|error| match seen.into_inner() {
                Some(seen) if error.downcast_ref::<crate::finding::OutOfTime>().is_some() => {
                    error.context(format!("last read {seen:?}"))
                }
                None if error.downcast_ref::<crate::finding::OutOfTime>().is_some() => {
                    error.context("nothing matched to read")
                }
                _ => error,
            })
        }

        /// Wait until exactly `expected` nodes match `by`.
        ///
        /// **Exact equality, polled, starting immediately -- which is a trap
        /// directly after an action.** A call whose `expected` is the count that
        /// was already on screen is answered by the very first poll, before the
        /// action it is meant to be checking can have landed, so it asserts only
        /// that the page has not caught up yet. It stays green for months and
        /// then turns into a hard timeout the first time anything, however
        /// unrelated, makes the page a few milliseconds slower to lay out --
        /// and blames that change. Where the count is supposed to have *changed*,
        /// read it before the action and say so with [`Self::wait_count_from`],
        /// which refuses the degenerate case instead of passing it. Plain
        /// `wait_count` belongs where a count is being established rather than
        /// changed: a page just loaded, a selector first asserted.
        pub async fn wait_count(&self, by: impl IntoBy, expected: usize) -> Result<()> {
            let by = by.into_by();

            tracing::trace!(?by, expected, "Waiting for count");

            self.wait_until(format_args!("{by} to match {expected} nodes"), async || {
                Ok(self.find_all(by.clone()).await?.len() == expected)
            })
            .await
        }

        /// How many nodes match `by` right now, without waiting for any
        /// particular number of them. The before-read [`Self::wait_count_from`]
        /// asks for.
        #[inline]
        pub async fn count(&self, by: impl IntoBy) -> Result<usize> {
            Ok(self.find_all(by.into_by()).await?.len())
        }

        /// Wait until exactly `expected` nodes match `by`, having been told the
        /// `before` count that the action in between was supposed to change.
        ///
        /// **The before-read is the whole point.** [`Self::wait_count`] cannot
        /// tell "the action landed" apart from "the action has not started",
        /// because both of those look exactly like the number it was given; this
        /// can, because it knows which number meant "not yet". When `before`
        /// already equals `expected` it refuses outright rather than passing on
        /// the first poll, so the assertion that verifies nothing fails loudly
        /// the day it is written instead of years later on somebody else's
        /// change. Take `before` with [`Self::count`] immediately before the
        /// action -- at the top of the test it is a different question, and
        /// anything landing in between makes it a lie.
        pub async fn wait_count_from(
            &self,
            by: impl IntoBy,
            before: usize,
            expected: usize,
        ) -> Result<()> {
            let by = by.into_by();

            ensure!(
                before != expected,
                "Waiting for {by} to match {expected} nodes verifies nothing: \
                 {expected} is also the count read before the action, so the \
                 first poll agrees whether or not the action has landed. Assert \
                 the count this action is supposed to produce, or if it really \
                 is meant to leave the count alone, assert what it did change.",
            );

            tracing::trace!(?by, before, expected, "Waiting for count to change");

            self.wait_until(
                format_args!("{by} to go from {before} to {expected} nodes"),
                async || Ok(self.find_all(by.clone()).await?.len() == expected),
            )
            .await
        }

        /// Find the text contents of all nodes match the given [`By`].
        #[inline]
        pub async fn find_all_texts(&self, by: impl IntoBy) -> Result<Vec<String>> {
            let mut texts = Vec::new();

            for item in self.find_all(by.into_by()).await? {
                texts.push(item.text().await?);
            }

            Ok(texts)
        }

        /// Find an element by xpath.
        #[inline]
        pub async fn find_one_by_xpath(&self, xpath: &str) -> Result<TestElement> {
            self.find_one_by(By::XPath(xpath)).await
        }

        /// Find the one element matching `by`, waiting for it to appear.
        ///
        /// **Refusing a second match is the point.** A selector that was meant
        /// to name one thing and now names three is the way a guard goes quiet,
        /// so this says so instead of picking. Where several *is* what the page
        /// is, the question was never "the one": see [`Self::find_nth`].
        pub async fn find_one_by(&self, by: impl IntoBy) -> Result<TestElement> {
            let by = by.into_by();

            tracing::trace!(?by, "Finding one element");

            crate::finding::poll(
                self.wait_timeout,
                || format!("Timed out after {:?} waiting for {by}", self.wait_timeout),
                async || {
                let items = self.find_all(by.clone()).await?;

                ensure!(
                    items.len() <= 1,
                    "Expected exactly one by {by}, but got {}: where several is \
                     what the page is, ask for one of them by index with \
                     find_nth, or for the first of them with find_first",
                    items.len()
                );

                Ok(items.into_iter().next())
            })
            .await
        }

        /// Find the `index`th element matching `by`, waiting for that many to
        /// appear.
        ///
        /// **What a test asks when several is the answer**, and the reading twin
        /// of [`TestDriver::press_nth`](crate::TestDriver::press_nth): the row a test appended is the one at
        /// the end, three related buttons are three whatever is measured, and
        /// neither is a broken selector for [`Self::find_one_by`] to refuse.
        /// Asked here rather than by reaching for [`Self::find_all`] and
        /// indexing the answer, which drops the wait along with the sentence
        /// saying which one was wanted.
        ///
        /// A wait polling for an element to *arrive* still wants
        /// [`Self::find_all`]: nothing there is an answer to that question, and
        /// this has a deadline of its own to spend before it says so.
        pub async fn find_nth(&self, by: impl IntoBy, index: usize) -> Result<TestElement> {
            let by = by.into_by();

            tracing::trace!(?by, index, "Finding one of several elements");

            crate::finding::poll(
                self.wait_timeout,
                || format!("Timed out after {:?} waiting for {by} at {index}", self.wait_timeout),
                async || Ok(self.find_all(by.clone()).await?.into_iter().nth(index)),
            )
            .await
        }

        /// The first element matching `by`, however many there turn out to be.
        /// [`Self::find_nth`] says when that is the question.
        #[inline]
        pub async fn find_first(&self, by: impl IntoBy) -> Result<TestElement> {
            self.find_nth(by, 0).await
        }

        /// Find all elements matching `by`.
        pub async fn find_all(&self, by: impl IntoBy) -> Result<Vec<TestElement>> {
            self.find_all_within(self.wait_timeout, by).await
        }

        /// [`Self::find_all`] with a bound of the caller's choosing on the call
        /// itself, for putting the question to a browser which is still
        /// starting. Inside a wait, the bound is never longer than what is left
        /// of that wait. See
        /// [`TestDriver::load_timeout`](crate::TestDriver::load_timeout).
        pub async fn find_all_within(
            &self,
            timeout: Duration,
            by: impl IntoBy,
        ) -> Result<Vec<TestElement>> {
            let by = by.into_by();
            let timeout = crate::finding::bounded(timeout);
            tracing::trace!(?by, "Finding all elements");

            let task = self.inner.find_all(by.clone());
            let sleep = time::sleep(timeout);

            tokio::select! {
                _ = sleep => {
                    Err(crate::finding::OutOfTime(format!(
                        "find_elements({by:?}) timed out after {timeout:?}"
                    ))
                    .into())
                }
                result = task => {
                    Ok(result?.into_iter().map(|inner| TestElement { inner, wait_timeout: self.wait_timeout, waiting: self.waiting.clone(), holding: self.holding.clone() }).collect::<Vec<_>>())
                }
            }
        }
    }
}

pub(crate) use common;

/// **Anything that names elements to find**, taken by every lookup on
/// [`TestDriver`](crate::TestDriver) that says `by`.
///
/// A string (`&str` or `&String`) is a CSS selector; any other way of finding
/// elements is written out as a [`By`].
///
/// # Examples
///
/// ```no_run
/// use yew_e2e::prelude::*;
///
/// async fn lookups(driver: &TestDriver) -> Result<()> {
///     // A CSS selector.
///     driver.wait_count("li.item", 3).await?;
///
///     // One that was built rather than written out.
///     let id = 7;
///     driver.find_one_by(&format!("[data-test=row-{id}]")).await?;
///
///     // Any other `By`.
///     driver.find_first(By::XPath("//button[text()='Save']")).await?;
///     Ok(())
/// }
/// ```
pub trait IntoBy {
    /// The [`By`] this names.
    fn into_by(self) -> By;
}

impl IntoBy for &str {
    #[inline]
    fn into_by(self) -> By {
        By::Css(self)
    }
}

/// A selector that was built rather than written out, such as one naming a
/// control whose `data-test` carries an identifier in it.
impl IntoBy for &String {
    #[inline]
    fn into_by(self) -> By {
        By::Css(self.as_str())
    }
}

impl IntoBy for By {
    #[inline]
    fn into_by(self) -> By {
        self
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow, bail};
    use thirtyfour::error::{WebDriverError, WebDriverErrorInfo, WebDriverErrorInner};
    use tokio::time::{self, Duration, Instant};

    use super::{OutOfTime, bounded, is_stale, poll};

    /// A lookup that takes `takes` to answer, bounded as `find_all_within`
    /// bounds one.
    async fn slow_find(takes: Duration) -> Result<Option<()>> {
        let bound = bounded(Duration::from_secs(5));

        tokio::select! {
            _ = time::sleep(bound) => Err(OutOfTime(format!("timed out after {bound:?}")).into()),
            _ = time::sleep(takes) => Ok(None),
        }
    }

    #[test]
    fn outside_a_wait_a_bound_is_left_alone() {
        assert_eq!(bounded(Duration::from_secs(5)), Duration::from_secs(5));
    }

    /// What WebDriver answers a read of an element that has left the page.
    fn stale() -> anyhow::Error {
        WebDriverError::from(WebDriverErrorInner::StaleElementReference(
            WebDriverErrorInfo::new(String::from("stale element reference")),
        ))
        .into()
    }

    /// The timeout every poll here is given.
    fn waited() -> String {
        String::from("the poll timed out")
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_lookup_does_not_carry_a_wait_past_its_deadline() -> Result<()> {
        let start = Instant::now();

        let result = poll(Duration::from_millis(300), waited, async || {
            slow_find(Duration::from_secs(3)).await
        })
        .await;

        let Err(error) = result else {
            bail!("a slow lookup answered");
        };

        assert!(error.downcast_ref::<OutOfTime>().is_some());
        assert_eq!(error.to_string(), "the poll timed out");
        assert_eq!(start.elapsed(), Duration::from_millis(300));
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_poll_inside_a_wait_gives_up_with_it() -> Result<()> {
        let start = Instant::now();

        let result = poll(Duration::from_millis(300), waited, async || {
            poll(
                Duration::from_secs(5),
                || String::from("the inner poll"),
                async || Ok(None::<()>),
            )
            .await
            .map(Some)
        })
        .await;

        let Err(error) = result else {
            bail!("a poll that never answers answered");
        };

        assert_eq!(error.to_string(), "the poll timed out");
        assert!(start.elapsed() < Duration::from_millis(400));
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_lookup_out_of_its_own_time_is_still_a_failure() -> Result<()> {
        let result = poll(Duration::from_secs(60), waited, async || {
            slow_find(Duration::from_secs(10)).await
        })
        .await;

        let Err(error) = result else {
            bail!("a lookup that ran out of its own time passed");
        };

        assert_eq!(error.to_string(), "timed out after 5s");
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn an_attempt_that_fails_is_not_retried() -> Result<()> {
        let mut attempts = 0;

        let result = poll(Duration::from_secs(1), waited, async || {
            attempts += 1;
            Err::<Option<()>, _>(anyhow!("broken"))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1);
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_stale_element_is_looked_for_again() -> Result<()> {
        let mut attempts = 0;

        let found = poll(Duration::from_secs(1), waited, async || {
            attempts += 1;

            if attempts < 3 {
                return Err(stale().context("reading the row"));
            }

            Ok(Some(attempts))
        })
        .await?;

        assert_eq!(found, 3);
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_poll_that_only_met_stale_elements_says_so_when_it_gives_up() -> Result<()> {
        let result = poll(Duration::from_millis(300), waited, async || {
            Err::<Option<()>, _>(stale().context("reading the row"))
        })
        .await;

        let Err(error) = result else {
            bail!("a poll that only met stale elements answered");
        };

        assert!(error.downcast_ref::<OutOfTime>().is_some());
        let said = format!("{error:#}");
        assert!(
            said.starts_with("last attempt met a stale element: reading the row: "),
            "{said}"
        );
        assert!(said.ends_with(": the poll timed out"), "{said}");
        Ok(())
    }

    #[test]
    fn only_a_stale_element_is_stale() {
        assert!(is_stale(&stale()));
        assert!(is_stale(&stale().context("reading the row")));
        assert!(!is_stale(&anyhow!("stale element reference")));
        assert!(!is_stale(&OutOfTime(String::from("late")).into()));
    }
}
