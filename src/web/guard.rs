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
    fetch_dest: Option<&str>,
) -> bool {
    // DNS rebinding: a hostile name pointed here still sends its own `Host`.
    if !host.is_some_and(is_loopback) {
        return false;
    }
    if is_get {
        return true;
    }
    // Clickjacking: a state-changing request made by a framed page (the headers of
    // `security_headers` keep the pages from being framed at all; this is the second lock).
    if fetch_dest.is_some_and(|dest| matches!(dest, "iframe" | "frame" | "embed" | "object")) {
        return false;
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
    if !allowed(
        is_get,
        get("host"),
        get("origin"),
        get("sec-fetch-site"),
        get("sec-fetch-dest"),
    ) {
        return Err(forbidden().into());
    }
    next.run(cx, body).await
}

/// No framing by another site (`frame-ancestors`, `X-Frame-Options` for old browsers); the
/// palette preview is an iframe of this same origin. `style-src` keeps `'unsafe-inline'` for
/// the `style=` attributes (bar widths, tag colours); `img-src data:` is for the SVG masks in
/// the stylesheet. No inline script is allowed: every script is a `/*.js` file.
const HEADERS: [(&str, &str); 4] = [
    ("x-frame-options", "SAMEORIGIN"),
    (
        "content-security-policy",
        "frame-ancestors 'self'; default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'",
    ),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
];

#[layer("/")]
pub async fn security_headers(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let mut response = next.run(cx, body).await?;
    for (name, value) in HEADERS {
        response
            .headers_mut()
            .insert(name, topcoat::router::HeaderValue::from_static(value));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::allowed as allowed_with_dest;

    /// `allowed` without a `Sec-Fetch-Dest`.
    fn allowed(is_get: bool, host: Option<&str>, origin: Option<&str>, site: Option<&str>) -> bool {
        allowed_with_dest(is_get, host, origin, site, None)
    }

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

    /// A state-changing request made from inside a frame is refused; a top-level one is not.
    #[test]
    fn a_post_from_a_frame_is_refused() {
        let host = Some("127.0.0.1:3000");
        for dest in ["iframe", "frame", "embed", "object"] {
            assert!(
                !allowed_with_dest(false, host, None, None, Some(dest)),
                "{dest}"
            );
        }
        assert!(allowed_with_dest(false, host, None, None, Some("document")));
        assert!(allowed_with_dest(false, host, None, None, Some("empty")));
        // A GET may be framed (the palette preview); the headers decide who may frame it.
        assert!(allowed_with_dest(true, host, None, None, Some("iframe")));
    }
}

/// The real router, in process (no socket): see `doc/review_tools/testing/testproof/`.
#[cfg(test)]
mod router_tests {
    use crate::web::router;
    use topcoat::router::{Body, Method, StatusCode, request::Request, to_bytes};

    fn send(
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
    ) -> (StatusCode, Vec<(String, String)>) {
        let mut req = Request::builder().method(method).uri(path);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let response = router().handle(req.body(Body::empty()).unwrap()).await;
                let status = response.status();
                let out = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let _ = to_bytes(response.into_body(), usize::MAX).await;
                (status, out)
            })
    }

    fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn every_response_carries_the_framing_and_sniffing_headers() {
        let (status, h) = send(Method::GET, "/style.css", &[("host", "127.0.0.1:3000")]);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(header(&h, "x-frame-options"), Some("SAMEORIGIN"));
        let csp = header(&h, "content-security-policy").unwrap();
        assert!(csp.contains("frame-ancestors 'self'") && csp.contains("default-src 'self'"));
        assert!(
            !csp.contains("script-src"),
            "scripts stay 'self' only: {csp}"
        );
        assert_eq!(header(&h, "x-content-type-options"), Some("nosniff"));
        assert_eq!(header(&h, "referrer-policy"), Some("no-referrer"));
    }

    #[test]
    fn a_framed_post_is_forbidden_before_its_handler_runs() {
        let host = ("host", "127.0.0.1:3000");
        let (status, _) = send(
            Method::POST,
            "/folders/pick",
            &[host, ("sec-fetch-dest", "iframe")],
        );
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
