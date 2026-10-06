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

/// **Repeat `attempt` until it answers, or `timeout` passes**, looking again
/// every [`POLL_INTERVAL`]. `None` is running out of time, for the caller to
/// say what it was waiting for.
///
/// A poll inside a wait gives up no later than that wait does, and every
/// lookup an attempt makes is bounded by what is left of the poll's deadline;
/// an attempt that failed by running out of that time ran out of time, and is
/// not a failure of its own.
pub(crate) async fn poll<T>(
    timeout: Duration,
    mut attempt: impl AsyncFnMut() -> Result<Option<T>>,
) -> Result<Option<T>> {
    let own = Instant::now() + timeout;
    let deadline = DEADLINE.try_with(|outer| own.min(*outer)).unwrap_or(own);

    DEADLINE
        .scope(deadline, async move {
            loop {
                match attempt().await {
                    Ok(Some(found)) => return Ok(Some(found)),
                    Ok(None) => {}
                    Err(error)
                        if error.downcast_ref::<OutOfTime>().is_some()
                            && Instant::now() >= deadline => {}
                    Err(error) => return Err(error),
                }

                if Instant::now() >= deadline {
                    return Ok(None);
                }

                time::sleep(POLL_INTERVAL).await;
            }
        })
        .await
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

            let held = crate::finding::poll(timeout, async || Ok(condition().await?.then_some(())))
                .await?;

            match held {
                Some(()) => Ok(()),
                None => Err(crate::finding::OutOfTime(format!(
                    "Timed out after {timeout:?} waiting for {what}"
                ))
                .into()),
            }
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

            let found = crate::finding::poll(self.wait_timeout, async || {
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
            .await?;

            found.ok_or_else(|| {
                crate::finding::OutOfTime(format!(
                    "Timed out after {:?} waiting for {by}",
                    self.wait_timeout
                ))
                .into()
            })
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

            let found = crate::finding::poll(self.wait_timeout, async || {
                Ok(self.find_all(by.clone()).await?.into_iter().nth(index))
            })
            .await?;

            found.ok_or_else(|| {
                crate::finding::OutOfTime(format!(
                    "Timed out after {:?} waiting for {by} at {index}",
                    self.wait_timeout
                ))
                .into()
            })
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
    use tokio::time::{self, Duration, Instant};

    use super::{OutOfTime, bounded, poll};

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

    #[tokio::test(start_paused = true)]
    async fn a_slow_lookup_does_not_carry_a_wait_past_its_deadline() -> Result<()> {
        let start = Instant::now();

        let found = poll(Duration::from_millis(300), async || {
            slow_find(Duration::from_secs(3)).await
        })
        .await?;

        assert!(found.is_none());
        assert_eq!(start.elapsed(), Duration::from_millis(300));
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_poll_inside_a_wait_gives_up_with_it() -> Result<()> {
        let start = Instant::now();

        let found = poll(Duration::from_millis(300), async || {
            match poll(Duration::from_secs(5), async || Ok(None::<()>)).await? {
                Some(()) => Ok(Some(())),
                None => Err(OutOfTime(String::from("the inner poll")).into()),
            }
        })
        .await?;

        assert!(found.is_none());
        assert!(start.elapsed() < Duration::from_millis(400));
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn a_lookup_out_of_its_own_time_is_still_a_failure() -> Result<()> {
        let result = poll(Duration::from_secs(60), async || {
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

        let result = poll(Duration::from_secs(1), async || {
            attempts += 1;
            Err::<Option<()>, _>(anyhow!("broken"))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1);
        Ok(())
    }
}
