//! The flash: a one-shot message for the next page, carried in a short-lived cookie that
//! `ui/toast.js` turns into a toast and deletes. One layer sets it for every POST, so no
//! handler knows about it: a POST that redirects (303) gets a success message, and a refused
//! POST sent by a form (`Sec-Fetch-Mode: navigate`) goes back to the page it came from with
//! the refusal as an error message. A script's `fetch` keeps its status. With
//! `notifications.enabled: false` nothing here happens.

use crate::config::config;
use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, HeaderValue, Next, StatusCode,
        error::BadRequestError,
        header::{LOCATION, SET_COOKIE},
        layer,
        request::{headers, method, uri},
        response::{IntoResponse, Response},
    },
};

/// The cookie's name; `toast.js` reads the same.
pub const COOKIE: &str = "iter_flash";

/// What a 500 says in its toast: the detail is on the terminal, never in a page.
const BROKEN: &str = "Something went wrong; see the terminal.";

/// A 400 whose message is not known (an answer, not an error, carried the status).
const REFUSED: &str = "This could not be done.";

/// The success message of a redirecting POST, by route pattern (`{}` is any one segment);
/// the first match wins, so a literal segment goes before a `{}` at the same place.
const MESSAGES: &[(&str, &str)] = &[
    ("/settings", "Settings saved."),
    ("/task/new", "Task created."),
    ("/task/{}/edit", "Task saved."),
    ("/project/{}/edit", "Project saved."),
    ("/org/{}/edit", "Organization saved."),
    ("/board/{}/edit", "Board saved."),
    ("/org/{}/share", "Report shared."),
    ("/org/{}/project/quick", "Project created."),
    ("/board/{}/project/quick", "Project created."),
    ("/board/{}/matrix", "Task moved."),
    ("/board/{}/schedule", "Task moved."),
    ("/task/{}/notes", "Note saved."),
    ("/task/{}/sessions", "Session saved."),
    ("/project/{}/notes/today", "Today's note is open."),
    ("/org/{}/notes/today", "Today's note is open."),
    ("/project/{}/wiki/accept", "Wiki changes accepted."),
    ("/project/{}/wiki/revert", "Wiki changes reverted."),
    ("/project/{}/wiki/ingest", "Wiki ingest started."),
    ("/task/{}/runs", "Run started."),
    ("/task/{}/runs/{}/run", "Run started."),
    ("/task/{}/runs/{}/cancel", "Run cancelled."),
    ("/task/{}/plan/{}", "Step updated."),
    ("/instructions", "Instruction created."),
    ("/instruction/{}", "Instruction saved."),
];

/// When no pattern matches.
const SAVED: &str = "Saved.";

/// Whether `path` is an instance of `pattern`.
fn matches(pattern: &str, path: &str) -> bool {
    let mut pattern = pattern.split('/');
    let mut path = path.split('/');
    loop {
        match (pattern.next(), path.next()) {
            (None, None) => return true,
            (Some("{}"), Some(segment)) if !segment.is_empty() => {}
            (Some(want), Some(got)) if want == got => {}
            _ => return false,
        }
    }
}

fn success_message(path: &str) -> &'static str {
    MESSAGES
        .iter()
        .find(|(pattern, _)| matches(pattern, path))
        .map_or(SAVED, |(_, message)| message)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Ok,
    Error,
}

/// `name=value; attributes`: the value is `ok:` or `error:` and the message, percent-encoded
/// (a cookie value takes no space, comma, semicolon or quote). Readable by `toast.js`, so not
/// `HttpOnly`; a minute is ample for the redirect it rides on.
fn cookie(kind: Kind, message: &str) -> String {
    let kind = match kind {
        Kind::Ok => "ok",
        Kind::Error => "error",
    };
    format!(
        "{COOKIE}={}; Path=/; Max-Age=60; SameSite=Strict",
        encode(&format!("{kind}:{message}"))
    )
}

fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The path (and query) of `referer` when it is a page of this server (`host`, plain http),
/// else `None`: going back anywhere else would be an open redirect.
fn back_to(referer: &str, host: &str) -> Option<String> {
    let (scheme, rest) = referer.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let (authority, path) = rest.find('/').map_or((rest, ""), |at| rest.split_at(at));
    if !authority.eq_ignore_ascii_case(host) {
        return None;
    }
    let path = path.split('#').next().unwrap_or_default();
    // `//other.site` and `/\other.site` are read as another host by browsers
    if path.starts_with("//") || path.starts_with("/\\") {
        return None;
    }
    Some(if path.is_empty() {
        "/".into()
    } else {
        path.into()
    })
}

/// The refusal's message: a 400's own text as its page says it (without the
/// "bad request: " lead), a 500's generic sentence.
fn refusal(error: &topcoat::Error, status: StatusCode) -> String {
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        return BROKEN.into();
    }
    error.downcast_ref::<BadRequestError>().map_or_else(
        || REFUSED.into(),
        |e| {
            let text = e.to_string();
            text.strip_prefix("bad request: ")
                .unwrap_or(&text)
                .to_string()
        },
    )
}

#[layer("/")]
pub async fn flash(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let result = next.run(cx, body).await;
    // read after the handler: the settings save that turns toasts on is confirmed by one,
    // and the one that turns them off leaves no cookie behind
    if method(cx).as_str() != "POST" || !config().notifications.enabled {
        return result;
    }
    let error = result.as_ref().err().cloned();
    let mut response = result.into_response(cx)?;
    let status = response.status();
    if status == StatusCode::SEE_OTHER {
        add_cookie(&mut response, Kind::Ok, success_message(uri(cx).path()));
        return Ok(response);
    }
    if !matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::INTERNAL_SERVER_ERROR
    ) {
        return Ok(response);
    }
    let h = headers(cx);
    let get = |name: &str| h.get(name).and_then(|v| v.to_str().ok());
    if get("sec-fetch-mode") != Some("navigate") {
        return Ok(response);
    }
    let Some(back) = get("referer")
        .zip(get("host"))
        .and_then(|(r, h)| back_to(r, h))
    else {
        return Ok(response);
    };
    let Ok(location) = HeaderValue::from_str(&back) else {
        return Ok(response);
    };
    let message = match &error {
        Some(error) => refusal(error, status),
        None if status == StatusCode::INTERNAL_SERVER_ERROR => BROKEN.into(),
        None => REFUSED.into(),
    };
    let mut response = (StatusCode::SEE_OTHER, [(LOCATION, location)], ()).into_response(cx)?;
    add_cookie(&mut response, Kind::Error, &message);
    Ok(response)
}

fn add_cookie(response: &mut Response, kind: Kind, message: &str) {
    if let Ok(value) = HeaderValue::from_str(&cookie(kind, message)) {
        response.headers_mut().append(SET_COOKIE, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_matches_whole_segments() {
        assert!(matches("/task/{}/edit", "/task/7/edit"));
        assert!(!matches("/task/{}/edit", "/task//edit"));
        assert!(!matches("/task/{}/edit", "/task/7/edit/more"));
        assert!(!matches("/task/{}", "/task/7/edit"));
        assert!(matches("/settings", "/settings"));
    }

    #[test]
    fn each_route_has_its_message_and_others_are_saved() {
        assert_eq!(success_message("/settings"), "Settings saved.");
        assert_eq!(success_message("/task/3/edit"), "Task saved.");
        assert_eq!(success_message("/task/3/runs/2/cancel"), "Run cancelled.");
        assert_eq!(success_message("/nowhere"), SAVED);
        for (_, message) in MESSAGES {
            assert!(!message.to_lowercase().contains("error"), "{message}");
        }
    }

    #[test]
    fn the_cookie_is_encoded_and_short_lived() {
        let c = cookie(Kind::Error, "Name; \"x\", é");
        assert!(
            c.starts_with("iter_flash=error%3AName%3B%20%22x%22%2C%20%C3%A9;"),
            "{c}"
        );
        assert!(c.ends_with("; Path=/; Max-Age=60; SameSite=Strict"));
        assert!(HeaderValue::from_str(&c).is_ok());
    }

    #[test]
    fn only_a_page_of_this_server_is_gone_back_to() {
        let host = "127.0.0.1:3000";
        assert_eq!(
            back_to("http://127.0.0.1:3000/task/1/edit?x=1", host).as_deref(),
            Some("/task/1/edit?x=1")
        );
        assert_eq!(back_to("http://127.0.0.1:3000", host).as_deref(), Some("/"));
        assert_eq!(back_to("https://127.0.0.1:3000/", host), None);
        assert_eq!(back_to("http://127.0.0.1:9999/", host), None);
        assert_eq!(back_to("http://evil.example/", host), None);
        assert_eq!(back_to("http://127.0.0.1:3000//evil.example/", host), None);
        assert_eq!(back_to("/task/1", host), None);
    }
}
