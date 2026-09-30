//! The static files (`/v2.css`, `/org.css`, `/board.js`, `/org.js`) are
//! compiled in and only change with the binary. Each is served with an ETag
//! (a hash of its text, computed once) and `Cache-Control: no-cache`, so the
//! browser revalidates on every page and gets a 304 with no body instead of
//! downloading the file again.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
        request::headers,
        response::{IntoResponse, Response},
    },
};

pub struct Asset {
    text: &'static str,
    content_type: &'static str,
    tag: OnceLock<String>,
}

impl Asset {
    pub const fn css(text: &'static str) -> Self {
        Self::new(text, "text/css; charset=utf-8")
    }

    pub const fn js(text: &'static str) -> Self {
        Self::new(text, "text/javascript; charset=utf-8")
    }

    const fn new(text: &'static str, content_type: &'static str) -> Self {
        Self {
            text,
            content_type,
            tag: OnceLock::new(),
        }
    }

    fn tag(&self) -> &str {
        self.tag.get_or_init(|| {
            let mut hasher = DefaultHasher::new();
            self.text.hash(&mut hasher);
            format!("\"{:016x}\"", hasher.finish())
        })
    }

    /// The file, or a bodiless 304 when the browser already has this version.
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
            self.text,
        )
            .into_response(cx)
    }
}

/// Whether an `If-None-Match` value (a list of tags, or `*`) names `tag`.
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
