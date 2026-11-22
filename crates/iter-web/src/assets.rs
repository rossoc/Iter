//! Static files are compiled in and served with an ETag and `no-cache`: the browser revalidates and gets a bodiless 304.

use crate::prelude::*;
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use topcoat::router::Body;
use topcoat::router::HeaderValue;
use topcoat::router::Method;
use topcoat::router::Methods;
use topcoat::router::Path;
use topcoat::router::Route;
use topcoat::router::RouteFuture;
use topcoat::router::RouteId;
use topcoat::router::StatusCode;
use topcoat::router::header::CACHE_CONTROL;
use topcoat::router::header::CONTENT_TYPE;
use topcoat::router::header::ETAG;
use topcoat::router::header::IF_NONE_MATCH;
use topcoat::router::request::headers;
use topcoat::router::response::IntoResponse;

/// Registers one `GET` route per `(path, asset)`.
pub fn serve(builder: RouterBuilder, table: &[(&'static str, &'static Asset)]) -> RouterBuilder {
    table.iter().fold(builder, |builder, (path, asset)| {
        builder.route(Static::new(path, asset))
    })
}

/// A `GET` route that answers with a compiled-in file.
struct Static {
    id: RouteId,
    path: Cow<'static, Path>,
    asset: &'static Asset,
}

impl Static {
    fn new(path: &'static str, asset: &'static Asset) -> Static {
        Static {
            id: RouteId::new(),
            path: topcoat::router::IntoPath::into_path(path),
            asset,
        }
    }
}

impl Route for Static {
    fn id(&self) -> RouteId {
        self.id
    }

    fn methods(&self) -> Methods<'_> {
        Methods::Only(&[Method::GET])
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn handle<'cx>(&'cx self, cx: &'cx Cx, _body: Body) -> RouteFuture<'cx> {
        Box::pin(async move { self.asset.respond(cx) })
    }
}

/// A JSON body, with `Cache-Control: no-store` when `no_store`.
pub fn json(cx: &Cx, body: String, no_store: bool) -> Result<Response> {
    let content = [(CONTENT_TYPE, HeaderValue::from_static("application/json"))];
    if !no_store {
        (content, body).into_response(cx)
    } else {
        (
            content,
            [(CACHE_CONTROL, HeaderValue::from_static("no-store"))],
            body,
        )
            .into_response(cx)
    }
}

pub struct Asset {
    bytes: &'static [u8],
    content_type: &'static str,
    tag: OnceLock<String>,
}

impl Asset {
    pub const fn css(text: &'static str) -> Self {
        Self::new(text.as_bytes(), "text/css; charset=utf-8")
    }

    pub const fn js(text: &'static str) -> Self {
        Self::new(text.as_bytes(), "text/javascript; charset=utf-8")
    }

    pub const fn icon(bytes: &'static [u8]) -> Self {
        Self::new(bytes, "image/x-icon")
    }

    const fn new(bytes: &'static [u8], content_type: &'static str) -> Self {
        Self {
            bytes,
            content_type,
            tag: OnceLock::new(),
        }
    }

    fn tag(&self) -> &str {
        self.tag.get_or_init(|| {
            let mut hasher = DefaultHasher::new();
            self.bytes.hash(&mut hasher);
            format!("\"{:016x}\"", hasher.finish())
        })
    }

    pub fn respond(&self, cx: &Cx) -> Result<Response> {
        let tag = self.tag();
        let cached = matches(headers(cx).get(IF_NONE_MATCH), tag);
        let head = [
            (CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (
                ETAG,
                HeaderValue::from_str(tag).expect("a quoted hex string"),
            ),
        ];
        if cached {
            return (StatusCode::NOT_MODIFIED, head, Body::empty()).into_response(cx);
        }
        (
            head,
            [(CONTENT_TYPE, HeaderValue::from_static(self.content_type))],
            self.bytes,
        )
            .into_response(cx)
    }
}

fn matches(value: Option<&HeaderValue>, tag: &str) -> bool {
    let Some(value) = value.and_then(|v| v.to_str().ok()) else {
        return false;
    };
    value
        .split(',')
        .map(|t| t.trim().trim_start_matches("W/"))
        .any(|t| t == "*" || t == tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_in_the_list_matches() {
        let value = |s: &'static str| Some(HeaderValue::from_static(s));
        assert!(matches(value("\"a\"").as_ref(), "\"a\""));
        assert!(matches(value("\"b\", W/\"a\"").as_ref(), "\"a\""));
        assert!(matches(value("*").as_ref(), "\"a\""));
        assert!(!matches(value("\"b\"").as_ref(), "\"a\""));
        assert!(!matches(None, "\"a\""));
    }
}
