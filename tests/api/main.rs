//! The public [`TestDriver`] and [`TestElement`] API, method group by method
//! group, each against a small page this suite's fixture serves itself.
//!
//! ```text
//! cargo test --test api
//! cargo test --test api -- pointer::
//! E2E_BROWSER=chrome cargo test --test api
//! ```
//!
//! A machine with no browser this crate can drive skips the whole suite rather
//! than failing it, so `cargo test --all-targets` passes there; naming one with
//! `--browser` or `E2E_BROWSER` makes its absence an error again.

use std::time::Duration;

use anyhow::{Result, ensure};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use yew_e2e::prelude::*;
use yew_e2e::{Config, Engine, Fixture, Frontend};

mod attributes;
mod canvas;
mod capture;
mod drag;
mod finding;
mod input;
mod keyboard;
mod navigation;
mod pointer;
mod scripts;
mod scroll;
mod storage;
mod style;
mod waits;
mod wheel;

/// One page of the suite, served at `/{name}`.
pub struct Page {
    /// Where it is served, and the `data-page` its body carries once its
    /// script has run.
    pub name: &'static str,
    /// Markup inside `<body>`, ahead of the log.
    pub body: &'static str,
    /// Script run once the body is in place, with `record(line)` to hand.
    pub script: &'static str,
}

const PAGES: &[&Page] = &[
    &attributes::PAGE,
    &canvas::PAGE,
    &capture::PAGE,
    &drag::PAGE,
    &finding::PAGE,
    &input::PAGE,
    &keyboard::PAGE,
    &navigation::PAGE,
    &pointer::PAGE,
    &scripts::PAGE,
    &scripts::COLLIDE,
    &scroll::PAGE,
    &storage::PAGE,
    &style::PAGE,
    &waits::PAGE,
    &wheel::PAGE,
];

/// What `/download.json` answers, as an attachment.
pub const DOWNLOAD: &str = r#"{"saved":true}"#;

/// Every page's shell: a log a page appends lines to with `record`, kept in a
/// corner out of the way of anything a test measures.
fn render(page: &Page) -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>{name}</title>
<style>
body {{ font: 16px sans-serif; }}
[data-test=log] {{ position: fixed; right: 0; bottom: 0; margin: 0; width: 300px; max-height: 300px; overflow: auto; font-size: 12px; }}
</style>
</head>
<body>
{body}
<ol data-test="log"></ol>
<script>
const record = line => {{
    const item = document.createElement("li");
    item.textContent = line;
    document.querySelector("[data-test=log]").append(item);
}};
</script>
<script>
{script}
</script>
<script>document.body.dataset.page = "{name}";</script>
</body>
</html>
"#,
        name = page.name,
        body = page.body,
        script = page.script,
    )
}

/// What the server answers for `path`: a status line, a content type, any
/// further headers, and the body.
fn respond(path: &str) -> (&'static str, &'static str, &'static str, String) {
    let path = path.split(['?', '#']).next().unwrap_or_default();

    if path == "/download.json" {
        return (
            "200 OK",
            "application/json",
            "content-disposition: attachment; filename=\"download.json\"\r\n",
            DOWNLOAD.to_owned(),
        );
    }

    let name = path.trim_start_matches('/');

    if name.is_empty() {
        let links = PAGES
            .iter()
            .map(|page| format!("<li><a href=\"/{0}\">{0}</a></li>", page.name))
            .collect::<String>();

        return (
            "200 OK",
            "text/html; charset=utf-8",
            "",
            format!("<!doctype html><html><body><ul>{links}</ul></body></html>"),
        );
    }

    match PAGES.iter().find(|page| page.name == name) {
        Some(page) => ("200 OK", "text/html; charset=utf-8", "", render(page)),
        None => (
            "404 Not Found",
            "text/plain",
            "",
            format!("no page {name:?}"),
        ),
    }
}

/// The pages above, served on a port of their own for one test.
pub struct Pages {
    port: u16,
    server: JoinHandle<()>,
}

impl Pages {
    /// Where `path` is on this fixture's server.
    pub fn address(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{path}", self.port)
    }

    /// Go to `page`, wait for its script to have run, and start watching it
    /// for errors, so anything it throws fails the test that opened it.
    pub async fn open(&self, driver: &TestDriver, page: &Page) -> Result<()> {
        driver.webdriver().goto(self.address(page.name)).await?;
        driver
            .find_one_by(&format!("body[data-page={}]", page.name))
            .await?;
        driver.watch_for_errors().await?;
        Ok(())
    }
}

impl Fixture for Pages {
    type Setup = ();

    fn config() -> Config {
        Config::default()
            .about("The public TestDriver and TestElement API, against pages of its own.")
            .frontend(Frontend::None)
            .lock_name("yew-e2e-api.lock")
            .sessions_dir("e2e-sessions-api")
    }

    async fn start((): ()) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let server = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut request = [0; 8192];
                    let Ok(n) = stream.read(&mut request).await else {
                        return;
                    };

                    let request = String::from_utf8_lossy(&request[..n]);
                    let path = request.split(' ').nth(1).unwrap_or("/");
                    let (status, kind, headers, body) = respond(path);

                    let response = format!(
                        "HTTP/1.1 {status}\r\ncontent-type: {kind}\r\n{headers}\
                         content-length: {}\r\ncache-control: no-store\r\n\
                         connection: close\r\n\r\n{body}",
                        body.len()
                    );

                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });

        Ok(Self { port, server })
    }

    fn url(&self) -> String {
        self.address("")
    }

    async fn quit(self) -> Result<()> {
        self.server.abort();
        Ok(())
    }
}

/// Every line the page's log holds, in one observation.
pub async fn log(driver: &TestDriver) -> Result<Vec<String>> {
    driver.rendered_texts("[data-test=log] li").await
}

/// Wait until the page's log holds `line`, wherever it is in it.
pub async fn logged(driver: &TestDriver, line: &str) -> Result<()> {
    let result = driver
        .wait_until(format_args!("the log to say {line:?}"), async || {
            Ok(log(driver).await?.iter().any(|seen| seen == line))
        })
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(error.context(format!("the log said {:?}", log(driver).await?))),
    }
}

/// Wait until the page's log reads exactly `lines`.
pub async fn logged_exactly<const N: usize>(driver: &TestDriver, lines: [&str; N]) -> Result<()> {
    driver.wait_texts("[data-test=log] li", lines).await
}

/// Run a script in the page and read back what it returns.
pub async fn eval(driver: &TestDriver, script: &str) -> Result<serde_json::Value> {
    Ok(driver
        .webdriver()
        .execute(script, Vec::new())
        .await?
        .json()
        .clone())
}

/// Two numbers agree to within `within`.
pub fn near(what: &str, seen: f64, expected: f64, within: f64) -> Result<()> {
    ensure!(
        (seen - expected).abs() <= within,
        "{what}: expected {expected} (within {within}), got {seen}"
    );
    Ok(())
}

/// A short deadline for a wait that is meant to give up.
pub const BRIEF: Duration = Duration::from_millis(300);

mod suite {
    use super::*;

    yew_e2e::harness! {
        Pages;
        waits::{
            wait_until_polls_until_the_condition_holds,
            wait_until_within_gives_up_naming_what_it_waited_for,
            wait_until_passes_on_the_condition_error,
            wait_texts_follows_the_page,
            wait_texts_gives_up_saying_what_it_last_read,
            wait_count_establishes_a_count,
            wait_count_from_follows_a_change,
            wait_count_from_refuses_a_count_that_did_not_change,
            element_waits_are_scoped_to_the_element,
            timeouts_are_ordered,
        },
        finding::{
            find_one_by_answers_the_one_match,
            find_one_by_refuses_several,
            find_one_by_waits_for_an_element_to_arrive,
            find_one_by_xpath_finds_by_xpath,
            find_nth_and_find_first_pick_by_index,
            find_nth_gives_up_naming_the_index,
            find_all_and_count_see_every_match,
            find_all_texts_reads_each_match,
            find_all_within_bounds_the_call,
            rendered_texts_reads_in_one_observation,
            element_finds_are_scoped_to_the_element,
            selectors_can_be_built_or_given_as_by,
        },
        attributes::{
            text_attr_and_prop_read_the_element,
            element_hands_back_the_webdriver_element,
            css_and_rect_measure_the_element,
            visible_follows_display,
            pressed_reads_aria_pressed,
            enabled_and_disabled_read_their_own_attributes,
            find_all_attrs_reads_every_match,
            bands_measure_what_is_drawn,
        },
        input::{
            send_keys_and_clear_edit_a_field,
            focus_blur_and_focused_attr_follow_focus,
            set_value_fires_the_named_event,
            set_text_replaces_the_text_node,
            set_text_refuses_an_element_without_text,
        },
        keyboard::{
            press_key_lands_on_the_page,
            hold_key_and_release_key_hold_across_observations,
            press_key_on_dispatches_any_named_key,
            press_key_with_ctrl_carries_the_code,
            press_key_with_shift_holds_shift,
        },
        pointer::{
            click_presses_the_centre,
            extend_click_holds_control,
            shift_click_holds_shift,
            click_by_presses_off_centre,
            double_click_is_one_gesture,
            double_click_by_is_off_centre,
            context_click_opens_the_context_menu,
            hover_rests_the_pointer,
            middle_click_presses_the_middle_button,
            press_nth_presses_by_index,
            context_press_nth_right_clicks_by_index,
            drag_by_holds_the_button_until_drop_held,
            drag_by_lands_on_a_short_offset,
            drag_from_by_begins_off_centre,
            move_held_carries_the_held_drag,
            alt_drag_by_holds_alt_until_drop_held_alt,
            cancel_pointer_cancels_without_releasing,
            release_all_input_lets_go_of_a_drag,
            middle_drag_by_pans_with_the_middle_button,
            pointer_drag_onto_releases_over_the_target,
        },
        drag::{
            drag_onto_runs_the_whole_html_drag,
            drag_over_stops_short_of_the_drop,
            drag_off_leaves_the_target,
            external_files_drop_onto_a_target,
            native_drag_over_carries_with_the_pointer,
        },
        wheel::{
            wheel_turns_down,
            wheel_across_turns_sideways,
            wheel_alt_holds_alt_over_the_middle,
            wheel_over_says_whether_the_page_took_it,
            wheel_consumed_says_whether_the_page_took_it,
        },
        scroll::{
            scroll_top_scrolls_a_container,
            scroll_to_far_end_scrolls_the_nearest_scroller,
            scroll_into_view_brings_an_element_into_view,
        },
        navigation::{
            reload_starts_the_page_again,
            reopen_with_sets_the_query,
            in_second_tab_runs_beside_the_first,
            in_second_window_runs_beside_the_first,
            in_second_tab_fails_on_what_the_page_threw,
            set_window_size_resizes_the_window,
            what_the_page_says_describes_the_page,
            local_clock_reads_the_browser_clock,
        },
        storage::{
            local_storage_round_trips,
            block_local_storage_denies_the_page,
        },
        scripts::{
            watch_for_errors_hears_a_throw,
            watch_for_errors_hears_a_rejection,
            watch_for_errors_keeps_out_of_the_page_globals,
            capture_clipboard_and_wait_copied,
            delay_websocket_sends_patches_and_restores_send,
            watch_mutations_counts_changes,
        },
        style::{
            set_style_and_set_class_force_styles,
            computed_reads_a_pseudo_element,
            animation_time_pauses_an_animation,
            animation_time_refuses_a_missing_animation,
            reduced_motion_reads_the_media_query,
            painted_color_alpha_and_color_shift_measure_colour,
            overflowing_finds_clipped_boxes,
            small_text_finds_small_text,
        },
        canvas::{
            canvas_size_reads_the_three_sizes,
            canvas_columns_scans_what_is_drawn,
            drawn_inside_sees_what_stands_over_a_point,
        },
        capture::{
            screenshot_png_is_a_png,
            screenshot_writes_a_file,
            snapshot_follows_e2e_shots,
            downloads_collects_saved_files,
        },
    }

    /// The harness's own entrypoint, which [`yew_e2e::harness!`] declares in
    /// this module.
    pub(super) fn run() -> ! {
        main()
    }
}

/// Whether this run should go ahead: a browser is named, one can be found, or
/// what was asked for needs none.
fn runnable() -> bool {
    let asked = std::env::args().skip(1).any(|arg| {
        matches!(arg.as_str(), "--list" | "--help" | "-h") || arg.starts_with("--browser")
    });

    asked || std::env::var_os("E2E_BROWSER").is_some() || Engine::detect().is_some()
}

fn main() {
    if !runnable() {
        println!(
            "skipping the api suite: no browser this crate can drive is here \
             (neither geckodriver nor chromedriver on PATH, nor a Chrome to fetch \
             one for). Name one with --browser or E2E_BROWSER to make this an error."
        );
        return;
    }

    suite::run()
}
