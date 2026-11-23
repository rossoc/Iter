//! The page that updates itself (R2b, R2c of task 302): `GET /live` streams the live version as
//! server-sent events, and every framed page loads `/live.js` and marks `<main>` as a live
//! region (`data-live`).
//!
//! Like `toast_tests`, each test runs again in a child process whose config folder and database
//! are a scratch folder, so the user's own are never read or written.

use super::toast_tests::{get, isolated_in};
use crate::router;
use futures_core::Stream;
use iter_core::{config::config, db::Db, fixtures};
use std::{
    pin::Pin,
    sync::mpsc,
    time::{Duration, Instant},
};
use topcoat::router::{Body, Method, StatusCode, request::Request};

/// What `GET /live` answered: its status, its content type and the `data:` lines of the events
/// read before the stream was dropped.
struct Events {
    status: StatusCode,
    content_type: String,
    data: Vec<String>,
    raw: String,
}

/// `GET /live` through the router, reading the stream until `wanted` events carried data or
/// `within` passed. `between` runs (on the test's thread) once the first event has come.
fn live_events(wanted: usize, within: Duration, between: impl FnOnce() + Send + 'static) -> Events {
    let (tx, rx) = mpsc::channel();
    let (first_tx, first_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        runtime.block_on(async move {
            let req = Request::builder()
                .method(Method::GET)
                .uri("/live")
                .header("host", "127.0.0.1:3000")
                .header("accept", "text/event-stream")
                .body(Body::empty())
                .expect("a request");
            let response = router().handle(req).await;
            let status = response.status();
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string();
            let mut stream = response.into_body().into_data_stream();
            let (mut raw, mut data, mut told) = (String::new(), Vec::new(), false);
            let deadline = Instant::now() + within;
            while status == StatusCode::OK && data.len() < wanted && Instant::now() < deadline {
                let next = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await;
                let Some(Ok(bytes)) = next else { break };
                raw.push_str(&String::from_utf8_lossy(&bytes));
                data = raw
                    .lines()
                    .filter_map(|l| l.strip_prefix("data:"))
                    .map(|d| d.trim().to_string())
                    .collect();
                if !data.is_empty() && !told {
                    told = true;
                    let _ = first_tx.send(());
                }
            }
            let _ = tx.send(Events {
                status,
                content_type,
                data,
                raw,
            });
        });
    });
    if first_rx.recv_timeout(within).is_ok() {
        between();
    }
    rx.recv_timeout(within + Duration::from_secs(5))
        .expect("GET /live answered (the stream may never have sent an event)")
}

/// R2b: `GET /live` is an event stream whose first event carries the version at once.
#[test]
fn live_answers_an_event_stream_with_the_version() {
    if isolated_in(
        module_path!(),
        "live_answers_an_event_stream_with_the_version",
        None,
    ) {
        return;
    }
    let events = live_events(1, Duration::from_secs(5), || {});
    assert_eq!(events.status, StatusCode::OK, "{}", events.raw);
    assert!(
        events.content_type.starts_with("text/event-stream"),
        "content type {:?}",
        events.content_type
    );
    let first = events.data.first().unwrap_or_else(|| {
        panic!("a first event with the version: {:?}", events.raw)
    });
    assert!(!first.is_empty(), "the version: {:?}", events.raw);
}

/// R2b: a commit to iter.db from another connection (the CLI) is sent as a new version within
/// a few of the stream's one-second checks.
#[test]
fn live_sends_a_new_version_after_a_commit() {
    if isolated_in(
        module_path!(),
        "live_sends_a_new_version_after_a_commit",
        None,
    ) {
        return;
    }
    let path = config().db_path().expect("a scratch database path");
    drop(Db::open(&path).expect("opens"));
    let events = live_events(2, Duration::from_secs(8), move || {
        let db = Db::open(&path).expect("opens");
        fixtures::add_project(&db, "app");
    });
    assert_eq!(events.status, StatusCode::OK, "{}", events.raw);
    assert!(
        events.data.len() >= 2,
        "a second event after the commit: {:?}",
        events.raw
    );
    assert_ne!(
        events.data[0], events.data[1],
        "the version changed: {:?}",
        events.raw
    );
}

/// R2c: every framed page loads `/live.js` and its `<main>` is a live region; the script is
/// served and listens on `/live` for the `[data-live]` regions.
#[test]
fn every_page_loads_live_js_and_marks_main_live() {
    if isolated_in(
        module_path!(),
        "every_page_loads_live_js_and_marks_main_live",
        None,
    ) {
        return;
    }
    let db = Db::open(config().db_path().expect("a scratch database path")).expect("opens");
    let project = fixtures::add_project(&db, "app");
    assert_eq!(fixtures::add_task(&db, project, "Fix it"), 1);
    drop(db);
    for path in [
        "/",
        "/settings",
        "/agents",
        "/boards",
        "/instructions",
        "/project/1",
        "/task/1",
        "/task/1?tab=notes",
        "/task/1?tab=agent",
        "/task/1/edit",
    ] {
        let page = get(path);
        assert_eq!(page.status, StatusCode::OK, "{path}: {}", page.body);
        assert!(
            page.body.contains(r#"<script src="/live.js""#),
            "{path} loads live.js: {}",
            page.body
        );
        let main = page.body.find("<main").expect("a <main>");
        let tag = &page.body[main..main + page.body[main..].find('>').expect("closed")];
        assert!(
            tag.contains(r#"id="main""#) && tag.contains("data-live"),
            "{path}: <main> is live: {tag}"
        );
    }

    let script = get("/live.js");
    assert_eq!(script.status, StatusCode::OK);
    for wanted in ["EventSource", "/live", "data-live"] {
        assert!(script.body.contains(wanted), "{wanted} in live.js");
    }
}
