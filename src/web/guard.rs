//! Keeps the editor from being driven by anything but the user's own browser tab:
//! `Host` must be loopback and a state-changing `Origin` must be this server
//! (CSRF, DNS rebinding). The threat model is in `doc/web_server.md`.

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
    // DNS rebinding: a hostile name pointed here still sends its own `Host`.
    if !host.is_some_and(is_loopback) {
        return false;
    }
    if is_get {
        return true;
    }
    // Names a cross-site request even if `Origin` was stripped; a page cannot forge it.
    if fetch_site.is_some_and(|site| !matches!(site, "same-origin" | "none")) {
        return false;
    }
    match origin {
        Some(origin) => origin.split_once("://").is_some_and(|(_, rest)| {
            is_loopback(rest) && host.is_some_and(|h| h.eq_ignore_ascii_case(rest))
        }),
        // Neither header: not a browser tab (a page's request carries one), so `curl` works.
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
