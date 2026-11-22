//! Tests of the notification toasts (sketch `plan`, R3-R8): the flash cookie a POST sets, the
//! refused navigational POST sent back with an error flash, the toast region and its assets.
//!
//! They go through the real router in process, as `guard.rs`'s `router_tests` do. A POST to
//! `/settings` writes the config file, so each test runs again in a child process of this test
//! binary whose `$XDG_CONFIG_HOME` and `$HOME` are a scratch folder: the user's own
//! `iter.yaml` and database are never read or written.

use crate::web::router;
use std::path::Path;
use std::process::Command;
use topcoat::router::{Body, Method, StatusCode, request::Request, to_bytes};

/// Set in the child: the test body runs there.
const CHILD: &str = "ITER_TOAST_TEST_CHILD";

const HOST: &str = "127.0.0.1:3000";
const ORIGIN: &str = "http://127.0.0.1:3000";

/// What a browser's `<form method="post">` on `/settings` sends.
const FROM_A_TAB: [(&str, &str); 6] = [
    ("host", HOST),
    ("origin", ORIGIN),
    ("sec-fetch-site", "same-origin"),
    ("sec-fetch-mode", "navigate"),
    ("sec-fetch-dest", "document"),
    ("content-type", "application/x-www-form-urlencoded"),
];

/// A valid settings form, toasts on for `seconds`.
fn settings_form(notifications: bool, seconds: &str) -> String {
    let mut body = String::from(
        "editor=nvim&pause_gap_minutes=17&status=queue&branch_template=feat%2F%7Btask%7D\
         &default_branch=main&github_project=&tmux=on&auto_branch=on",
    );
    if notifications {
        body.push_str("&notifications=on");
    }
    body.push_str("&notification_seconds=");
    body.push_str(seconds);
    body
}

/// In the parent: runs test `name` again in a child whose config folder is a scratch one
/// holding `yaml` as `iter.yaml` (none if `None`), asserts it passed, and returns `true`
/// (the caller then returns). In the child: `false`, and the caller runs its body.
fn isolated(name: &str, yaml: Option<&str>) -> bool {
    isolated_in(module_path!(), name, yaml)
}

/// [`isolated`] for a test of another module: `module` is that file's `module_path!()`.
pub(super) fn isolated_in(module: &str, name: &str, yaml: Option<&str>) -> bool {
    if std::env::var_os(CHILD).is_some() {
        crate::config::load().expect("the scratch config loads");
        return false;
    }
    let home = crate::fixtures::scratch(&format!("toast-{name}"));
    let config_home = home.join("config");
    let dir = config_home.join("iter");
    std::fs::create_dir_all(&dir).expect("scratch config folder");
    if let Some(yaml) = yaml {
        std::fs::write(dir.join("iter.yaml"), yaml).expect("scratch config");
    }
    let module = module.split_once("::").map_or("", |(_, m)| m);
    let test = format!("{module}::{name}");
    let out = Command::new(std::env::current_exe().expect("the test binary"))
        .args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", &home)
        .output()
        .expect("the test binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let saved = read(&dir.join("iter.yaml"));
    let _ = std::fs::remove_dir_all(&home);
    assert!(
        out.status.success() && stdout.contains("1 passed"),
        "{test} in a scratch config folder failed:\n--- stdout\n{stdout}\n--- stderr\n{stderr}\n--- iter.yaml\n{saved}"
    );
    true
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub(super) struct Answer {
    pub(super) status: StatusCode,
    headers: Vec<(String, String)>,
    pub(super) body: String,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn cookies(&self) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k == "set-cookie")
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// The one cookie set (the flash): its decoded value and its attributes, lowercased.
    fn flash(&self) -> (String, String) {
        let cookies = self.cookies();
        assert_eq!(
            cookies.len(),
            1,
            "one flash cookie is set ({} {:?})",
            self.status,
            self.headers
        );
        let (pair, attributes) = cookies[0].split_once(';').unwrap_or((cookies[0], ""));
        let value = pair.split_once('=').map_or("", |(_, v)| v);
        (percent_decode(value), attributes.to_ascii_lowercase())
    }

    /// The page with its `<main>` cut out: the frame around it, where the toasts live.
    fn outside_main(&self) -> String {
        let start = self.body.find("<main").expect("a page with a <main>");
        let end = self.body.find("</main>").expect("a closed <main>");
        format!("{}{}", &self.body[..start], &self.body[end..])
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = text.get(i + 1..i + 3);
        if bytes[i] == b'%'
            && let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn send(method: Method, path: &str, headers: &[(&str, &str)], body: &str) -> Answer {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = req.body(Body::from(body.to_string())).expect("a request");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(async {
            let response = router().handle(req).await;
            let status = response.status();
            let headers = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let bytes = to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap_or_default();
            Answer {
                status,
                headers,
                body: String::from_utf8_lossy(&bytes).into_owned(),
            }
        })
}

pub(super) fn get(path: &str) -> Answer {
    send(Method::GET, path, &[("host", HOST)], "")
}

/// A POST from a tab on `referer` (`None`: no `Referer`), as a form (`navigate`) or a
/// script's `fetch` (`cors`).
fn post(path: &str, body: &str, mode: &str, referer: Option<&str>) -> Answer {
    let mut headers: Vec<(&str, &str)> = FROM_A_TAB
        .iter()
        .copied()
        .map(|(k, v)| {
            if k == "sec-fetch-mode" {
                (k, mode)
            } else {
                (k, v)
            }
        })
        .collect();
    if let Some(referer) = referer {
        headers.push(("referer", referer));
    }
    send(Method::POST, path, &headers, body)
}

fn assert_flash_cookie_attributes(attributes: &str) {
    for wanted in ["path=/", "samesite=strict", "max-age=60"] {
        assert!(attributes.contains(wanted), "{wanted} in {attributes:?}");
    }
    assert!(
        !attributes.contains("httponly"),
        "toast.js reads it: {attributes:?}"
    );
}

/// A refused POST that touches nothing: `x` is not a task id, so the answer is a 400.
const REFUSED: &str = "/task/x/edit";

/// R3: a saved form redirects (303) and the answer sets the short-lived, script-readable
/// flash cookie with a success message.
#[test]
fn a_saved_form_redirects_with_a_success_flash() {
    if isolated("a_saved_form_redirects_with_a_success_flash", None) {
        return;
    }
    let answer = post(
        "/settings",
        &settings_form(true, "5"),
        "navigate",
        Some("http://127.0.0.1:3000/settings"),
    );
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    let (value, attributes) = answer.flash();
    assert_flash_cookie_attributes(&attributes);
    assert!(
        !value.trim().is_empty() && !value.to_lowercase().contains("error"),
        "a success message: {value:?}"
    );
}

/// R4: a POST refused with a 400 from a form goes back (303) to the same-origin page it came
/// from, with an error flash carrying the refusal's own message; a script's `fetch` still
/// gets the 400 and its message, and no cookie.
#[test]
fn a_refused_form_goes_back_with_an_error_flash() {
    if isolated("a_refused_form_goes_back_with_an_error_flash", None) {
        return;
    }
    let from = "http://127.0.0.1:3000/task/1/edit";

    let fetched = post(REFUSED, "name=x", "cors", Some(from));
    assert_eq!(fetched.status, StatusCode::BAD_REQUEST, "{}", fetched.body);
    assert!(fetched.cookies().is_empty(), "{:?}", fetched.headers);
    let message = fetched.body.trim().to_string();
    assert!(!message.is_empty(), "the refusal says why");

    let navigated = post(REFUSED, "name=x", "navigate", Some(from));
    assert_eq!(
        navigated.status,
        StatusCode::SEE_OTHER,
        "{}",
        navigated.body
    );
    let location = navigated.header("location").unwrap_or_default();
    assert!(
        location == from || location == "/task/1/edit",
        "back to the page it came from: {location:?}"
    );
    let (value, attributes) = navigated.flash();
    assert_flash_cookie_attributes(&attributes);
    assert!(value.contains("error"), "an error flash: {value:?}");
    let said = message.strip_prefix("bad request: ").unwrap_or(&message);
    assert!(
        value.contains(said),
        "the flash carries the refusal {said:?}: {value:?}"
    );
}

/// R4, the edge: without a `Referer`, or with one from another site, there is nowhere safe to
/// go back to, and the refusal stays the bare 400 page (no open redirect).
#[test]
fn a_refused_form_without_a_same_origin_referer_is_not_redirected() {
    if isolated(
        "a_refused_form_without_a_same_origin_referer_is_not_redirected",
        None,
    ) {
        return;
    }
    for referer in [
        None,
        Some("http://evil.example/task/1/edit"),
        Some("https://127.0.0.1:3000/task/1/edit"),
        Some("http://127.0.0.1:9999/task/1/edit"),
    ] {
        let answer = post(REFUSED, "name=x", "navigate", referer);
        assert_eq!(
            answer.status,
            StatusCode::BAD_REQUEST,
            "referer {referer:?}"
        );
        assert!(answer.header("location").is_none(), "referer {referer:?}");
    }
}

/// R7: with `enabled: false` a saved form sets no cookie, and a refused form raises no toast.
#[test]
fn turned_off_there_is_no_cookie_and_no_toast() {
    if isolated(
        "turned_off_there_is_no_cookie_and_no_toast",
        Some("notifications:\n  enabled: false\n  seconds: 5\n"),
    ) {
        return;
    }
    let saved = post(
        "/settings",
        &settings_form(false, "5"),
        "navigate",
        Some("http://127.0.0.1:3000/settings"),
    );
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    assert!(saved.cookies().is_empty(), "{:?}", saved.headers);

    let refused = post(
        "/settings",
        &settings_form(false, "5").replace("editor=nvim", "editor="),
        "navigate",
        Some("http://127.0.0.1:3000/settings"),
    );
    assert_eq!(refused.status, StatusCode::OK);
    assert!(refused.body.contains("Editor cannot be empty."));
    assert!(
        !refused.outside_main().contains("Editor cannot be empty."),
        "no toast when turned off"
    );
}

/// R5 (and R2's fields): every framed page has the top-right toast region, announced
/// (`aria-live`), with the configured seconds as a `data-` attribute, and loads `/toast.js`,
/// which is served and defines `window.iterToast`; the stylesheet has the two kinds.
#[test]
fn every_page_has_the_toast_region_and_its_script() {
    if isolated(
        "every_page_has_the_toast_region_and_its_script",
        Some("notifications:\n  enabled: true\n  seconds: 7\n"),
    ) {
        return;
    }
    let pages = ["/settings", "/agents", "/boards"].map(|path| (path, get(path)));
    for (path, page) in &pages {
        assert_eq!(page.status, StatusCode::OK, "{path}: {}", page.body);
    }
    for (path, page) in &pages {
        let frame = page.outside_main();
        // the search palette has a live region already: the toasts' is the one with the seconds
        let at = frame
            .find("data-seconds=\"7\"")
            .unwrap_or_else(|| panic!("{path}: the configured seconds on the region"));
        let open = frame[..at].rfind('<').expect("inside a tag");
        let close = at + frame[at..].find('>').expect("a closed tag");
        assert!(
            frame[open..close].contains("aria-live"),
            "{path}: the region is live: {}",
            &frame[open..=close]
        );
        assert!(
            page.body.contains("<script src=\"/toast.js\""),
            "{path}: loads toast.js"
        );
    }

    let settings = get("/settings");
    assert!(settings.body.contains("name=\"notifications\""));
    assert!(settings.body.contains("name=\"notification_seconds\""));

    let script = get("/toast.js");
    assert_eq!(script.status, StatusCode::OK);
    assert!(
        script
            .header("content-type")
            .is_some_and(|t| t.starts_with("text/javascript"))
    );
    assert!(script.body.contains("iterToast"));

    let css = get("/style.css").body;
    assert!(css.contains(".toast.ok") && css.contains(".toast.error"));
}

/// R6: a form sent back with an error also raises a red toast with the same message, beside
/// the inline error box (which stays, in `<main>`).
#[test]
fn a_form_error_also_raises_a_toast() {
    if isolated("a_form_error_also_raises_a_toast", None) {
        return;
    }
    let refused = post(
        "/settings",
        &settings_form(true, "5").replace("editor=nvim", "editor="),
        "navigate",
        Some("http://127.0.0.1:3000/settings"),
    );
    assert_eq!(refused.status, StatusCode::OK);
    let main_start = refused.body.find("<main").expect("main");
    let main_end = refused.body.find("</main>").expect("main");
    assert!(
        refused.body[main_start..main_end].contains("Editor cannot be empty."),
        "the inline error box stays"
    );
    assert!(
        refused.outside_main().contains("Editor cannot be empty."),
        "the toast carries the message"
    );
}

/// R8: the settings page no longer shows its own "Saved." notice; the toast says it.
#[test]
fn the_settings_page_has_no_saved_notice() {
    if isolated("the_settings_page_has_no_saved_notice", None) {
        return;
    }
    let page = get("/settings?saved=1");
    assert_eq!(page.status, StatusCode::OK);
    assert!(!page.body.contains("id=\"saved\""), "no inline notice");
}

/// R11: the save that turns the toasts on is told so with a blue toast: the layer reads
/// `notifications.enabled` after the handler has saved the new config.
#[test]
fn the_save_that_turns_toasts_on_shows_one() {
    if isolated(
        "the_save_that_turns_toasts_on_shows_one",
        Some("notifications:\n  enabled: false\n  seconds: 5\n"),
    ) {
        return;
    }
    let saved = post(
        "/settings",
        &settings_form(true, "5"),
        "navigate",
        Some("http://127.0.0.1:3000/settings"),
    );
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    let (value, attributes) = saved.flash();
    assert_flash_cookie_attributes(&attributes);
    assert!(
        !value.trim().is_empty() && !value.to_lowercase().contains("error"),
        "a success message: {value:?}"
    );
}

/// R11, the reverse: the save that turns the toasts off sets no cookie, though they were on
/// when the request came in.
#[test]
fn the_save_that_turns_toasts_off_sets_no_cookie() {
    if isolated(
        "the_save_that_turns_toasts_off_sets_no_cookie",
        Some("notifications:\n  enabled: true\n  seconds: 5\n"),
    ) {
        return;
    }
    let saved = post(
        "/settings",
        &settings_form(false, "5"),
        "navigate",
        Some("http://127.0.0.1:3000/settings"),
    );
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    assert!(saved.cookies().is_empty(), "{:?}", saved.headers);
}

/// R12: a refused daily-note edit raises the red toast like every other form: the message is
/// in the inline box (in `<main>`) and in the frame's toast region (outside it).
#[test]
fn a_refused_daily_note_raises_a_toast() {
    if isolated("a_refused_daily_note_raises_a_toast", None) {
        return;
    }
    // in the child: `$HOME` is the scratch folder, and the database is the scratch config's
    let home = std::path::PathBuf::from(std::env::var_os("HOME").expect("a scratch home"));
    let notes = home.join("notes");
    let id = crate::app::App::open()
        .expect("opens")
        .db
        .insert(&crate::models::Project {
            notes_path: notes.display().to_string(),
            ..crate::fixtures::project("app")
        })
        .expect("project inserts");
    let day = crate::fixtures::day("2026-01-05");
    crate::notes::create(&notes, day).expect("the note exists");

    let path = format!("/project/{id}/notes/2026-01-05");
    let referer = format!("{ORIGIN}{path}?tab=edit");
    // front matter opened and never closed: `notes::check` refuses it
    let refused = post(
        &path,
        "text=---%0Atitle%3A+x%0A",
        "navigate",
        Some(&referer),
    );
    assert_eq!(refused.status, StatusCode::OK, "{}", refused.body);
    let main_start = refused.body.find("<main").expect("main");
    let main_end = refused.body.find("</main>").expect("main");
    let wanted = "front matter is not closed";
    assert!(
        refused.body[main_start..main_end].contains(wanted),
        "the inline error box: {}",
        &refused.body[main_start..main_end]
    );
    assert!(
        refused.outside_main().contains(wanted),
        "the toast carries the message"
    );
}
