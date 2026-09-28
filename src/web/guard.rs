//! Keeps the editor from being driven by anything but the user's own
//! browser tab.
//!
//! The server listens on loopback, but a page on *any* website can still
//! make the user's browser POST to `127.0.0.1` (CSRF), or point a hostile
//! DNS name at it (rebinding). Both are closed by checking two headers the
//! browser sets and a web page can't forge: `Host` must be a loopback name,
//! and a state-changing request's `Origin`, when sent, must be this server.

use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Next, error::forbidden, layer, request::headers, request::method,
        response::Response,
    },
};

fn host_of(value: &str) -> &str {
    // Strip `:port` (there are no bracketed IPv6 hosts we accept).
    value.rsplit_once(':').map_or(value, |(host, _)| host)
}

fn is_loopback(host: &str) -> bool {
    matches!(host_of(host), "127.0.0.1" | "localhost")
}

/// Whether a request with these headers should be served.
fn allowed(is_get: bool, host: Option<&str>, origin: Option<&str>) -> bool {
    if !host.is_some_and(is_loopback) {
        return false;
    }
    if is_get {
        return true;
    }
    match origin {
        Some(origin) => origin
            .split_once("://")
            .is_some_and(|(_, rest)| is_loopback(rest) && Some(rest) == host),
        None => true,
    }
}

#[layer("/")]
pub async fn local_only(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let h = headers(cx);
    let get = |name: &str| h.get(name).and_then(|v| v.to_str().ok());
    let is_get = matches!(method(cx).as_str(), "GET" | "HEAD");
    if !allowed(is_get, get("host"), get("origin")) {
        return Err(forbidden().into());
    }
    next.run(cx, body).await
}

#[cfg(test)]
mod tests {
    use super::allowed;

    #[test]
    fn only_loopback_hosts_are_served() {
        assert!(allowed(true, Some("127.0.0.1:3000"), None));
        assert!(allowed(true, Some("localhost:3000"), None));
        assert!(!allowed(true, Some("evil.example:3000"), None));
        assert!(!allowed(true, None, None));
    }

    #[test]
    fn posts_must_come_from_this_server() {
        let host = Some("127.0.0.1:3000");
        assert!(allowed(false, host, Some("http://127.0.0.1:3000")));
        assert!(allowed(false, host, None));
        assert!(!allowed(false, host, Some("https://evil.example")));
        assert!(!allowed(false, host, Some("http://127.0.0.1:9999")));
    }
}
