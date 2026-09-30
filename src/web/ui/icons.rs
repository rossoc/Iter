//! The inline SVG icons. Lucide (ISC), https://lucide.dev/license -- paths
//! pasted inline, except `HALF` (a half-filled circle for work in progress),
//! drawn here. Stroke, size and color come from `.v2 .i` in `v2.css`. A page
//! that needs another icon adds one constant here.

use topcoat::view::Unescaped;

pub const KANBAN: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 3v14"/><path d="M12 3v8"/><path d="M19 3v18"/></svg>"#;
pub const FOLDER: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;
pub const ARROW: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7h10v10"/><path d="M7 17 17 7"/></svg>"#;
pub const MARK: &str = r#"<svg class="i mark" viewBox="0 0 24 24" aria-hidden="true"><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/></svg>"#;
pub const PENCIL: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/></svg>"#;
pub const CHECK: &str =
    r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>"#;
pub const MINUS: &str =
    r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14"/></svg>"#;
pub const CIRCLE: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/></svg>"#;
pub const HALF: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 3a9 9 0 0 1 0 18z" fill="currentColor"/></svg>"#;
pub const CIRCLE_CHECK: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="m16 9-5.5 5.5L8 12"/></svg>"#;
pub const COPY: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>"#;
pub const CLIPBOARD_CHECK: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><path d="m9 14 2 2 4-4"/></svg>"#;
pub const PLUS: &str = r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14"/><path d="M12 5v14"/></svg>"#;

pub const CHEVRON_LEFT: &str =
    r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="m15 18-6-6 6-6"/></svg>"#;
pub const CHEVRON_RIGHT: &str =
    r#"<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="m9 18 6-6-6-6"/></svg>"#;

pub fn icon(svg: &'static str) -> Unescaped<&'static str> {
    Unescaped::new_unchecked(svg)
}
