#[cfg(test)]
mod browser_death {
    use crate::browser_death;

    #[test]
    fn a_dead_session_says_its_own_name() {
        let error = anyhow::anyhow!(
            "invalid session id: Tried to run command without establishing a connection"
        );

        assert_eq!(browser_death(&error), Some("invalid session id"));
    }

    #[test]
    fn the_wrapper_says_it_without_the_state_line() {
        let error = anyhow::anyhow!("The WebDriver session id is invalid: something further in")
            .context("pressing the crossing");

        assert_eq!(browser_death(&error), Some("session id is invalid"));
    }

    #[test]
    fn a_death_buried_in_the_chain_is_still_found() {
        let error = anyhow::anyhow!("session deleted because of page crash")
            .context("waiting for the page to answer");

        assert_eq!(
            browser_death(&error),
            Some("session deleted because of page crash")
        );
    }

    #[test]
    fn a_driver_that_cannot_be_reached_is_also_a_death() {
        let error = anyhow::anyhow!("error sending request for url (http://127.0.0.1:9/session)")
            .context("finding the one element");

        assert_eq!(browser_death(&error), Some("error sending request"));
    }

    #[test]
    fn anything_else_stays_an_ordinary_failure() {
        for said in [
            "the log to be showing [\"e2e-log\"] did not agree, last read []",
            "the test was still running after 30s, which is --timeout",
            "the page threw: something a component said",
            "no such element: unable to locate the button",
        ] {
            let error = anyhow::anyhow!("{said}");
            assert_eq!(browser_death(&error), None, "{said} is not a death");
        }
    }
}

#[cfg(unix)]
mod lifecycle {
    use crate::{
        Browser,
        driver::{
            QUIT_TIMEOUT, STARTUP_TIMEOUT,
            lifecycle_tests::{fake, gone},
        },
    };
    use std::net::SocketAddr;
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
        sync::{mpsc, oneshot},
        task::JoinHandle,
        time::{Instant, timeout},
    };

    struct Request {
        line: String,
        reply: oneshot::Sender<Option<&'static str>>,
    }

    struct Server {
        address: SocketAddr,
        requests: mpsc::UnboundedReceiver<Request>,
        task: JoinHandle<()>,
    }

    impl Server {
        async fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (tx, requests) = mpsc::unbounded_channel();
            let task = tokio::spawn(async move {
                loop {
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut socket = BufReader::new(socket);
                    let mut line = String::new();
                    socket.read_line(&mut line).await.unwrap();
                    let mut length = 0;
                    loop {
                        let mut header = String::new();
                        socket.read_line(&mut header).await.unwrap();
                        if header == "\r\n" || header.is_empty() {
                            break;
                        }
                        if let Some(value) =
                            header.to_ascii_lowercase().strip_prefix("content-length:")
                        {
                            length = value.trim().parse().unwrap();
                        }
                    }
                    let mut body = vec![0; length];
                    socket.read_exact(&mut body).await.unwrap();
                    let (reply, response) = oneshot::channel();
                    if tx.send(Request { line, reply }).is_err() {
                        break;
                    }
                    if let Ok(Some(body)) = response.await {
                        let status = if body.contains("\"error\"") {
                            "500 Internal Server Error"
                        } else {
                            "200 OK"
                        };
                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                }
            });
            Self {
                address,
                requests,
                task,
            }
        }

        async fn request(&mut self, expected: &str) -> Request {
            let request = timeout(STARTUP_TIMEOUT, self.requests.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(request.line.starts_with(expected), "{}", request.line);
            request
        }

        /// Creating a session is two requests, not one: thirtyfour posts the
        /// session's timeouts as part of `WebDriver::new`. Leaving the second
        /// unanswered hangs startup until its deadline instead of handing back
        /// the browser these tests are about.
        async fn browser(&mut self) -> (Browser, i32) {
            let (driver, pid) = fake(self.address).await;
            let connecting = tokio::spawn(Browser::connect(
                driver,
                thirtyfour::DesiredCapabilities::firefox().into(),
                Instant::now() + STARTUP_TIMEOUT,
            ));
            self.request("POST /session ")
                .await
                .reply
                .send(Some(
                    r#"{"value":{"sessionId":"fake-session","capabilities":{}}}"#,
                ))
                .unwrap();
            self.request("POST /session/fake-session/timeouts ")
                .await
                .reply
                .send(Some(r#"{"value":null}"#))
                .unwrap();
            (connecting.await.unwrap().unwrap(), pid)
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    #[tokio::test]
    async fn a_pending_session_creation_times_out_and_reaps_the_driver() {
        let mut server = Server::new().await;
        let (driver, pid) = fake(server.address).await;
        let connecting = tokio::spawn(Browser::connect(
            driver,
            thirtyfour::DesiredCapabilities::firefox().into(),
            Instant::now() + STARTUP_TIMEOUT,
        ));
        let _pending = server.request("POST /session ").await;
        tokio::time::pause();
        tokio::time::advance(STARTUP_TIMEOUT).await;
        tokio::time::resume();
        let error = connecting.await.unwrap().err().unwrap();
        let message = format!("{error:#}");
        assert!(message.contains("startup timed out"), "{message}");
        assert!(message.contains("Listening on"), "{message}");
        gone(pid).await;
    }

    #[tokio::test]
    async fn a_failed_session_creation_retains_its_protocol_error() {
        let mut server = Server::new().await;
        let (driver, pid) = fake(server.address).await;
        let connecting = tokio::spawn(Browser::connect(
            driver,
            thirtyfour::DesiredCapabilities::firefox().into(),
            Instant::now() + STARTUP_TIMEOUT,
        ));
        server.request("POST /session ").await.reply.send(Some(r#"{"value":{"error":"session not created","message":"startup-original-marker","stacktrace":""}}"#)).unwrap();
        let error = connecting.await.unwrap().err().unwrap();
        assert!(format!("{error:#}").contains("startup-original-marker"));
        gone(pid).await;
    }

    #[tokio::test]
    async fn a_hung_quit_is_reaped_and_only_warned_about() {
        let mut server = Server::new().await;
        let (browser, pid) = server.browser().await;
        let closing = tokio::spawn(browser.close());
        let _pending = server.request("DELETE /session/fake-session ").await;
        tokio::time::pause();
        tokio::time::advance(QUIT_TIMEOUT).await;
        tokio::time::resume();
        let warning = closing
            .await
            .unwrap()
            .unwrap()
            .expect("a slow quit is said");
        assert!(format!("{warning:#}").contains("quit timed out"));
        gone(pid).await;
        assert!(
            server.requests.try_recv().is_err(),
            "Drop must not retry DELETE"
        );
    }

    #[tokio::test]
    async fn a_crashed_session_is_reaped_and_keeps_its_error() {
        let mut server = Server::new().await;
        let (browser, pid) = server.browser().await;
        let closing = tokio::spawn(browser.close());
        server.request("DELETE /session/fake-session ").await.reply.send(Some(r#"{"value":{"error":"invalid session id","message":"browser-crashed-marker","stacktrace":""}}"#)).unwrap();
        let error = closing.await.unwrap().unwrap_err();
        assert!(format!("{error:#}").contains("browser-crashed-marker"));
        gone(pid).await;
        assert!(server.requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn cancelling_startup_or_quit_still_reaps_owned_processes() {
        let mut server = Server::new().await;
        let (driver, pid) = fake(server.address).await;
        let connecting = tokio::spawn(Browser::connect(
            driver,
            thirtyfour::DesiredCapabilities::firefox().into(),
            Instant::now() + STARTUP_TIMEOUT,
        ));
        let pending = server.request("POST /session ").await;
        connecting.abort();
        assert!(matches!(connecting.await, Err(error) if error.is_cancelled()));
        gone(pid).await;
        drop(pending);

        let (browser, pid) = server.browser().await;
        let closing = tokio::spawn(browser.close());
        let _pending = server.request("DELETE /session/fake-session ").await;
        closing.abort();
        assert!(closing.await.unwrap_err().is_cancelled());
        gone(pid).await;
        assert!(server.requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn successful_quit_also_reaps_the_owned_driver() {
        let mut server = Server::new().await;
        let (browser, pid) = server.browser().await;
        let closing = tokio::spawn(browser.close());
        server
            .request("DELETE /session/fake-session ")
            .await
            .reply
            .send(Some(r#"{"value":null}"#))
            .unwrap();
        assert!(closing.await.unwrap().unwrap().is_none());
        gone(pid).await;
    }
}
