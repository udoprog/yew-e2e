//! A suite that runs as it is: the fixture serves one page of its own on a
//! port of its own, so there is no frontend to build.
//!
//! ```text
//! cargo run -p yew-e2e --example static_page
//! cargo run -p yew-e2e --example static_page -- --list
//! cargo run -p yew-e2e --example static_page -- --headed counter::
//! ```

use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use yew_e2e::{Config, Fixture, Frontend};

const PAGE: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>Counter</title></head>
<body>
<h1 data-test="title">Hello</h1>
<output data-test="count">0</output>
<button data-test="add">Add</button>
<button data-test="reset" hidden>Reset</button>
<script>
const count = document.querySelector("[data-test=count]");
const reset = document.querySelector("[data-test=reset]");
document.querySelector("[data-test=add]").addEventListener("click", () => {
    count.textContent = String(Number(count.textContent) + 1);
    reset.hidden = false;
});
reset.addEventListener("click", () => {
    count.textContent = "0";
    reset.hidden = true;
});
document.body.dataset.page = "counter";
</script>
</body>
</html>
"#;

/// What a test can ask of the page it is handed.
#[derive(Default)]
struct Setup {
    /// Start with the title reading something else.
    renamed: bool,
}

/// One server, serving [`PAGE`] to whatever asks.
struct Page {
    port: u16,
    server: JoinHandle<()>,
}

impl Fixture for Page {
    type Setup = Setup;

    fn config() -> Config {
        Config::default()
            .about("An example suite against a static page.")
            .frontend(Frontend::None)
            .sessions_dir("e2e-sessions-static-page")
    }

    async fn start(setup: Setup) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let page = match setup.renamed {
            true => PAGE.replace(">Hello<", ">Goodbye<"),
            false => PAGE.to_owned(),
        };

        let server = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let page = page.clone();

                tokio::spawn(async move {
                    let mut request = [0; 4096];
                    let _ = stream.read(&mut request).await;

                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}",
                        page.len()
                    );

                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });

        Ok(Self { port, server })
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    async fn quit(self) -> Result<()> {
        self.server.abort();
        Ok(())
    }
}

mod greeting {
    use yew_e2e::prelude::*;

    use super::Page;

    pub async fn says_hello(driver: &mut TestDriver, _: &mut Page) -> Result<()> {
        driver.wait_texts("[data-test=title]", ["Hello"]).await
    }

    pub async fn says_goodbye_when_renamed(driver: &mut TestDriver, _: &mut Page) -> Result<()> {
        driver.wait_texts("[data-test=title]", ["Goodbye"]).await
    }
}

mod counter {
    use yew_e2e::prelude::*;

    use super::Page;

    pub async fn counts_presses(driver: &mut TestDriver, _: &mut Page) -> Result<()> {
        driver.watch_for_errors().await?;

        let add = driver.find_one_by("[data-test=add]").await?;
        add.click().await?;
        add.click().await?;

        driver.wait_texts("[data-test=count]", ["2"]).await
    }

    pub async fn reset_shows_once_there_is_something_to_reset(
        driver: &mut TestDriver,
        _: &mut Page,
    ) -> Result<()> {
        ensure!(
            !driver
                .find_one_by("[data-test=reset]")
                .await?
                .visible()
                .await?,
            "reset shows before anything was counted"
        );

        driver.press_nth("[data-test=add]", 0).await?;
        driver.press_nth("[data-test=reset]", 0).await?;

        driver.wait_texts("[data-test=count]", ["0"]).await
    }
}

yew_e2e::harness! {
    Page;
    greeting::{says_hello, says_goodbye_when_renamed(renamed)},
    counter::{counts_presses, reset_shows_once_there_is_something_to_reset},
}
