//! PROOF: topcoat 0.9 lets iter's web layer be tested in-process, no socket.
//! `Router::handle(Request) -> Response` (topcoat-router-0.9.0/src/router.rs:73),
//! `Request` = `http::Request<Body>` (request.rs:19), `to_bytes` (body.rs:217).
use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Next, OriginPolicy, Router, RouterBuilder, layer, page, route,
        error::forbidden, request::headers, request::method, response::Response,
        response::IntoResponse, content::Html,
    },
    view::{View, view},
};

// ---- copied verbatim from iter src/web/guard.rs:13-58 ----
fn host_of(value: &str) -> &str {
    value.rsplit_once(':').map_or(value, |(host, _)| host)
}
fn is_loopback(host: &str) -> bool {
    matches!(host_of(host), "127.0.0.1" | "localhost")
}
fn allowed(is_get: bool, host: Option<&str>, origin: Option<&str>, fetch_site: Option<&str>) -> bool {
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
// ---- end of copy ----

/// A page whose text comes from "user data" and must be escaped by `view!`.
#[page("/")]
async fn home() -> Result<impl View> {
    let name = "<b>x</b> & \"q\"";
    Ok(view! { <h1>(name)</h1> })
}

#[route(POST "/task/new")]
async fn create(cx: &Cx, _body: Body) -> Result<Response> {
    Html("<p>created</p>".to_string()).into_response(cx)
}

fn router(framework_origin_check: bool) -> Router {
    let mut b = RouterBuilder::new().layer(local_only).page(home).route(create);
    if !framework_origin_check {
        // Take topcoat's own OriginPolicy out so a 403 can only come from the copied guard.
        b = b.origin_policy(OriginPolicy::dangerous_disable());
    }
    b.build()
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    use topcoat::router::{Method, StatusCode, request::Request, to_bytes};

    fn block_on<F: Future>(f: F) -> F::Output {
        // `block_in_place` (iter's blocking.rs) would need the multi-thread runtime; a plain one does here.
        tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(f)
    }

    fn send(router: &Router, method: Method, path: &str, hdrs: &[(&str, &str)]) -> (StatusCode, String) {
        let mut req = Request::builder().method(method).uri(path);
        for (k, v) in hdrs {
            req = req.header(*k, *v);
        }
        let response = block_on(router.handle(req.body(Body::empty()).unwrap()));
        let status = response.status();
        let bytes = block_on(to_bytes(response.into_body(), usize::MAX)).unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[test]
    fn a_get_renders_html_with_user_text_escaped() {
        let (status, body) = send(&router(true), Method::GET, "/", &[("host", "127.0.0.1:3000")]);
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("&lt;b&gt;x&lt;/b&gt;"), "{body}");
        assert!(!body.contains("<b>x</b>"), "{body}");
        println!("GET / => {status}\n{body}");
    }

    #[test]
    fn a_foreign_host_is_forbidden_by_the_guard_alone() {
        // topcoat's OriginPolicy never looks at Host on a GET, so this 403 is the guard's.
        let (status, body) = send(&router(true), Method::GET, "/", &[("host", "evil.example")]);
        assert_eq!((status, body.as_str()), (StatusCode::FORBIDDEN, "forbidden"));
    }

    #[test]
    fn a_cross_origin_post_is_forbidden_by_the_guard_with_the_framework_check_off() {
        let r = router(false);
        let host = ("host", "127.0.0.1:3000");
        let (s, _) = send(&r, Method::POST, "/task/new", &[host, ("origin", "https://evil.example")]);
        assert_eq!(s, StatusCode::FORBIDDEN);
        let (s, _) = send(&r, Method::POST, "/task/new", &[host, ("sec-fetch-site", "cross-site")]);
        assert_eq!(s, StatusCode::FORBIDDEN);
        let (s, body) = send(&r, Method::POST, "/task/new", &[host, ("origin", "http://127.0.0.1:3000")]);
        assert_eq!((s, body.as_str()), (StatusCode::OK, "<p>created</p>"));
        // Gap the Oct review named (M3): the scheme is not compared.
        let (s, _) = send(&r, Method::POST, "/task/new", &[host, ("origin", "https://127.0.0.1:3000")]);
        assert_eq!(s, StatusCode::OK, "scheme mismatch is accepted today");
    }

    #[test]
    fn an_unmatched_path_is_a_404_and_the_guard_does_not_run_there() {
        let (s, _) = send(&router(false), Method::GET, "/nope", &[("host", "evil.example")]);
        assert_eq!(s, StatusCode::NOT_FOUND, "a layer at \"/\" wraps matched routes only");
    }
}
