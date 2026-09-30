//! The page's title block: a kicker above the `h1`, actions on the right.

use super::Crumb;
use super::breadcrumb::breadcrumb;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The line above the title: a plain label, or where the page sits.
#[derive(Default)]
pub enum Kicker<'a> {
    #[default]
    None,
    Eyebrow(&'a str),
    Crumbs(&'a [Crumb]),
}

/// The child nodes are the actions (`edit_button`, ...), on the right.
#[component]
pub async fn page_header(
    title: &str,
    #[default] kicker: Kicker<'_>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class="head">
            <div>
                match kicker {
                    Kicker::Eyebrow(text) => <p class="label">(text)</p>,
                    Kicker::Crumbs(items) => breadcrumb(items: items),
                    Kicker::None => "",
                }
                <h1>(title)</h1>
            </div>
            <div class="head-actions">(child)</div>
        </div>
    })
}

/// The line under a `page_header`.
#[component]
pub async fn lede(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <p class="lede">(child)</p> })
}
