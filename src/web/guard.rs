//! Keeps the editor from being driven by anything but the user's own
//! browser tab.
//!
//! The server listens on loopback, but a page on *any* website can still
//! make the user's browser POST to `127.0.0.1` (CSRF), or point a hostile
//! DNS name at it (rebinding). Both are closed by checking two headers the
//! browser sets and a web page can't forge: `Host` must be a loopback name,
//! and a state-changing request's `Origin`, when sent, must be this server.
//! `Sec-Fetch-Site` backs the `Origin` up: a browser that sends it names a
//! cross-site request even if `Origin` was stripped, and it cannot be forged
//! by a page. A request with neither header is not from a browser tab (a
//! page's request always carries one), so it is let through for `curl`.

use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Next, error::forbidden, layer, request::headers, request::method, response::Response,
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
fn allowed(
    is_get: bool,
    host: Option<&str>,
    origin: Option<&str>,
    fetch_site: Option<&str>,
) -> bool {
    if !host.is_some_and(is_loopback) {
        return false;
    }
    if is_get {
        return true;
    }
    if fetch_site.is_some_and(|site| !matches!(site, "same-origin" | "none")) {
        return false;
    }
    match origin {
        Some(origin) => origin.split_once("://").is_some_and(|(_, rest)| {
            is_loopback(rest) && host.is_some_and(|h| h.eq_ignore_ascii_case(rest))
        }),
        None => true,
    }
}

#[layer("/")]
pub async fn local_only(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let h = headers(cx);
    let get = |name: &str| h.get(name).and_then(|v| v.to_str().ok());
    let is_get = matches!(method(cx).as_str(), "GET" | "HEAD");
    if !allowed(is_get, get("host"), get("origin"), get("sec-fetch-site")) {
        return Err(forbidden().into());
    }
    next.run(cx, body).await
}

#[cfg(test)]
mod tests {
    use super::allowed;

    #[test]
    fn only_loopback_hosts_are_served() {
        assert!(allowed(true, Some("127.0.0.1:3000"), None, None));
        assert!(allowed(true, Some("localhost:3000"), None, None));
        assert!(!allowed(true, Some("evil.example:3000"), None, None));
        assert!(!allowed(true, None, None, None));
    }

    #[test]
    fn posts_must_come_from_this_server() {
        let host = Some("127.0.0.1:3000");
        assert!(allowed(
            false,
            host,
            Some("http://127.0.0.1:3000"),
            Some("same-origin")
        ));
        assert!(allowed(false, host, None, None));
        assert!(!allowed(false, host, Some("https://evil.example"), None));
        assert!(!allowed(false, host, Some("http://127.0.0.1:9999"), None));
    }

    /// A browser that stripped `Origin` still says the request is cross-site.
    #[test]
    fn a_cross_site_post_is_refused_even_without_an_origin() {
        let host = Some("127.0.0.1:3000");
        assert!(!allowed(false, host, None, Some("cross-site")));
        assert!(!allowed(false, host, None, Some("same-site")));
        assert!(allowed(false, host, None, Some("none")));
        assert!(allowed(true, host, None, Some("cross-site")));
    }
}
