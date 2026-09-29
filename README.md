# yew-e2e

[<img alt="github" src="https://img.shields.io/badge/github-udoprog/yew--e2e-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/yew-e2e)
[<img alt="crates.io" src="https://img.shields.io/crates/v/yew-e2e.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/yew-e2e)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-yew--e2e-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/yew-e2e)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/udoprog/yew-e2e/ci.yml?branch=main&style=for-the-badge" height="20">](https://github.com/udoprog/yew-e2e/actions?query=branch%3Amain)

Browser tests for web applications, and the runner that drives them.

Written for [Yew] applications built with [Trunk], though nothing here depends
on either: a suite names the [`Fixture`] that starts the application under test
for every test, and each test is handed a [`TestDriver`] pointed at it.

What the runner takes care of:

* One binary and one entrypoint in place of libtest, declared with
  [`harness!`], running several browsers at once (`--parallel`).
* Firefox through `geckodriver` or Chrome through `chromedriver`, with a
  `chromedriver` matching the installed Chrome fetched when none is on the
  `PATH`.
* The frontend built once per run with `trunk build`, or served from `E2E_DIST`
  when it is already built.
* A record of every run, so `--last-session` runs only what did not pass.
* One run at a time per build directory (`--lock`).
* Waits that say what they were waiting for when they give up, and a report of
  what the page was doing when it never came up.

<br>

## Example

A test is an `async fn` taking the driver and the fixture. The fixture starts
the application for one test and says where the browser should go:

```rust
use yew_e2e::prelude::*;
use yew_e2e::{Config, Fixture, Frontend};

struct App {
    url: String,
}

impl Fixture for App {
    type Setup = ();

    fn config() -> Config {
        Config::default().frontend(Frontend::None)
    }

    async fn start((): ()) -> Result<Self> {
        // Start the application here, on a port of its own.
        Ok(App { url: String::from("http://127.0.0.1:8080") })
    }

    fn url(&self) -> String {
        self.url.clone()
    }

    async fn quit(self) -> Result<()> {
        Ok(())
    }
}

mod greeting {
    use yew_e2e::prelude::*;

    pub async fn says_hello<F>(driver: &mut TestDriver, _: &mut F) -> Result<()> {
        driver.wait_texts("h1", ["Hello"]).await
    }
}

yew_e2e::harness! {
    App;
    greeting::{says_hello},
}
```

The suite is registered as a test without libtest's harness:

```toml
[[test]]
name = "e2e"
path = "tests/e2e/main.rs"
harness = false
```

A test that needs something other than the bare fixture names it in
parentheses after its name, each word setting a `bool` field of the fixture's
`Setup`:

```rust
yew_e2e::harness! {
    App;
    cart::{adds_an_item, removes_an_item},
    login::{remembers_the_user(signed_in)},
}
```

[`examples/static_page.rs`] is a suite that runs as it is, against a page its
fixture serves itself:

```sh
cargo run -p yew-e2e --example static_page
```

<br>

## The frontend

By default the fixture's frontend is built once per run with `trunk build`, in
the workspace root, into `target/e2e-dist`. [`dist()`] says where it is, for
the fixture to serve. [`Frontend::Trunk(dir)`] runs it somewhere else under
the workspace root, and [`Frontend::None`] builds nothing, for a fixture that
serves its own pages.

Everything else a run leaves behind goes under the build directory too: the
record of every run in `target/e2e-sessions`, the run lock, a fetched
`chromedriver` in `target/e2e`. Two suites sharing one build directory give
each other room with [`Config::sessions_dir`] and [`Config::lock_name`].

<br>

## Running

```sh
cargo test -p app-e2e                      # everything
cargo test -p app-e2e -- cart::            # everything whose name contains cart::
cargo test -p app-e2e -- --exact cart::adds_an_item
cargo test -p app-e2e -- --list            # the names, one per line, on stdout
cargo test -p app-e2e -- --last-session    # what did not pass last time
```

| Flag | |
|---|---|
| `-j`, `--parallel N` | How many tests run at once, each in its own browser. Defaults to 2. |
| `--headed` | Show the browsers. |
| `--browser firefox\|chrome` | Which browser to drive. Detected where not given. |
| `--exact` | Filters match whole names rather than parts of them. |
| `--list` | List the selected tests and run nothing. |
| `--allow-empty` | A run that selects nothing passes rather than fails. |
| `--last-session`, `--session NAME` | Run only what did not pass in that session. |
| `--timeout SECONDS` | How long one test may run. Defaults to 30; 0 waits for ever. |
| `--grace SECONDS` | How long a running test is given to finish after Ctrl-C or SIGTERM. |
| `--lock target\|project\|none` | Which other runs this one waits for: those in the same build directory (the default), every worktree of the Git project, or none. |
| `--lock-wait SECONDS` | How long to wait for that lock. |

| Variable | |
|---|---|
| `E2E_HEADED` | As `--headed`. |
| `E2E_BROWSER` | As `--browser`. |
| `E2E_DIST` | A frontend that is already built, served in place of building one. |
| `E2E_THEME` | `dark` or `light`, for the colour scheme the browser reports (Firefox). |
| `E2E_MOTION` | `reduce` or `normal`, for the motion preference the browser reports (Firefox). |
| `E2E_SCALE` | The device pixel ratio the browser reports. |
| `E2E_SHOTS` | Where [`TestDriver::snapshot`] writes; relative to `target/e2e-shots`. |

[`Config::lock_name`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.lock_name
[`Config::sessions_dir`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.Config.html#method.sessions_dir
[`dist()`]: https://docs.rs/yew-e2e/latest/yew_e2e/fn.dist.html
[`examples/static_page.rs`]: https://github.com/udoprog/yew-e2e/blob/main/examples/static_page.rs
[`Fixture`]: https://docs.rs/yew-e2e/latest/yew_e2e/trait.Fixture.html
[`Frontend::None`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.None
[`Frontend::Trunk(dir)`]: https://docs.rs/yew-e2e/latest/yew_e2e/enum.Frontend.html#variant.Trunk
[`harness!`]: https://docs.rs/yew-e2e/latest/yew_e2e/macro.harness.html
[`TestDriver::snapshot`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.TestDriver.html#method.snapshot
[`TestDriver`]: https://docs.rs/yew-e2e/latest/yew_e2e/struct.TestDriver.html
[Trunk]: https://trunkrs.dev
[Yew]: https://yew.rs
