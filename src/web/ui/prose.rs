//! An entity's description: markdown rendered as HTML (`prose.css`). The text
//! is untrusted: raw HTML is shown as text, and links/images keep their URL only
//! if `http(s)`, `mailto` or relative. Headings move down two levels under the page's `<h1>`/`<h2>`.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use topcoat::{
    Result,
    view::{Unescaped, View, component, view},
};

/// CommonMark plus tables, strikethrough and task lists.
fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

fn shifted(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H3,
        HeadingLevel::H2 => HeadingLevel::H4,
        HeadingLevel::H3 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

/// A URL a link or image may keep: web or mail scheme, or none.
fn safe_url(url: &str) -> bool {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .filter(|scheme| !scheme.contains(['/', '?', '#']));
    match scheme {
        None => true,
        Some(scheme) => ["http", "https", "mailto"]
            .iter()
            .any(|ok| scheme.eq_ignore_ascii_case(ok)),
    }
}

/// `text` as HTML: raw HTML escaped, unsafe links dropped to their text, headings shifted.
pub fn markdown(text: &str) -> String {
    // whether each open link/image was kept, so its end matches
    let mut kept = Vec::new();
    let events = Parser::new_ext(text, options()).filter_map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Heading {
            level,
            id,
            classes,
            attrs,
        }) => Some(Event::Start(Tag::Heading {
            level: shifted(level),
            id,
            classes,
            attrs,
        })),
        Event::End(TagEnd::Heading(level)) => Some(Event::End(TagEnd::Heading(shifted(level)))),
        Event::Start(Tag::Link { ref dest_url, .. })
        | Event::Start(Tag::Image { ref dest_url, .. }) => {
            let keep = safe_url(dest_url);
            kept.push(keep);
            keep.then_some(event)
        }
        Event::End(TagEnd::Link) | Event::End(TagEnd::Image) => {
            kept.pop().unwrap_or(false).then_some(event)
        }
        other => Some(other),
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

/// Nothing when `text` is empty. `class`: `desc`, or the report's `pdesc` / `tdesc`.
#[component]
pub async fn description(text: &str, #[default("desc")] class: &str) -> Result<impl View> {
    let html = markdown(text);
    Ok(view! {
        if !text.trim().is_empty() {
            <div class=(format!("prose {class}"))>(Unescaped::new_unchecked(html))</div>
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_renders_blocks_and_inlines() {
        let html = markdown("Some *text* and `code`.\n\n- [ ] one\n- [x] two\n");
        assert!(html.contains("<p>Some <em>text</em> and <code>code</code>.</p>"));
        assert!(html.contains("<ul>"));
        assert!(html.contains("type=\"checkbox\""));
    }

    #[test]
    fn headings_sit_under_the_page_headings() {
        assert_eq!(markdown("# A\n"), "<h3>A</h3>\n");
        assert_eq!(markdown("### C\n"), "<h5>C</h5>\n");
        assert_eq!(markdown("##### E\n"), "<h6>E</h6>\n");
    }

    #[test]
    fn raw_html_is_text() {
        let html = markdown("<script>alert(1)</script>\n\nhi <b>there</b>");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(html.contains("&lt;b&gt;"), "{html}");
    }

    #[test]
    fn only_safe_links_keep_their_url() {
        let html =
            markdown("[ok](https://x.org) [rel](/org/1) [bad](javascript:alert(1)) ![i](data:x)");
        assert!(html.contains("<a href=\"https://x.org\">ok</a>"), "{html}");
        assert!(html.contains("<a href=\"/org/1\">rel</a>"), "{html}");
        assert!(!html.contains("javascript"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("bad"), "{html}");
    }

    #[test]
    fn a_url_without_a_scheme_is_safe() {
        assert!(safe_url("#top") && safe_url("../a") && safe_url("a/b:c"));
        assert!(safe_url("HTTPS://x") && safe_url("mailto:a@b"));
        assert!(!safe_url("javascript:x") && !safe_url(" data:x") && !safe_url("vbscript:x"));
    }
}
