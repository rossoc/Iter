//! The footer every page ends with: a large wordmark and, as a second menu,
//! the organizations with their projects, the boards, the projects that are
//! in no organization, the link to the global settings and the project's
//! link. No models here: `web/footer.rs` builds the data. Pure markup and
//! CSS, no script.

use topcoat::{
    Result,
    view::{View, component, view},
};

/// A link with its text.
pub struct FooterLink {
    pub name: String,
    pub href: String,
}

/// An organization: its link and its projects' (compact, muted). `projects`
/// may be the first few of `projects_total`; the rest end the list as a link
/// to the organization's page.
pub struct FooterGroup {
    pub link: FooterLink,
    pub projects: Vec<FooterLink>,
    pub projects_total: usize,
}

/// Everything the footer shows. Each list may be cut to its first few
/// links, with its total; a cut list ends with a "N more…" link to its
/// `_href`.
pub struct FooterData {
    pub orgs: Vec<FooterGroup>,
    pub boards: Vec<FooterLink>,
    pub boards_total: usize,
    pub boards_href: &'static str,
    /// Projects that belong to no organization.
    pub loose: Vec<FooterLink>,
    pub loose_total: usize,
    pub loose_href: &'static str,
    /// The page of the global settings.
    pub settings_href: String,
    pub version: &'static str,
}

/// Where the project lives.
pub const GITHUB: &str = "https://github.com/rossoc/Iter";

/// The text of the link that ends a cut list: how many of its `total` the
/// `shown` links leave out. `None` when nothing is cut.
fn more(shown: usize, total: usize) -> Option<String> {
    (total > shown).then(|| format!("{} more\u{2026}", total - shown))
}

/// A column's title and a flat list of links, or `none` when there are no
/// links. A list cut short (`total` above the links shown) ends with the
/// link to `more_href`.
#[component]
async fn link_column(
    id: &str,
    title: &str,
    links: &[FooterLink],
    none: &str,
    total: usize,
    more_href: &str,
) -> Result<impl View> {
    Ok(view! {
        <section class="foot-col" aria-labelledby=(id)>
            <h2 id=(id) class="label">(title)</h2>
            if links.is_empty() {
                <p class="foot-none">(none)</p>
            } else {
                <ul>
                    for l in links.iter() {
                        <li><a class="u" href=(l.href.as_str())>(l.name.as_str())</a></li>
                    }
                    if let Some(text) = more(links.len(), total) {
                        <li><a class="foot-more" href=(more_href)>(text)</a></li>
                    }
                </ul>
            }
        </section>
    })
}

#[component]
pub async fn site_footer(data: &FooterData) -> Result<impl View> {
    Ok(view! {
        <footer class="site-foot">
            <div class="foot-inner">
                <div class="foot-brand">
                    <a class="foot-mark" href="/">"iter"</a>
                    <p>"Plan the day, track the work."</p>
                </div>
                <div class="foot-cols">
                    <section class="foot-col" aria-labelledby="foot-orgs">
                        <h2 id="foot-orgs" class="label">"Organizations"</h2>
                        if data.orgs.is_empty() {
                            <p class="foot-none">"No organizations yet."</p>
                        } else {
                            <ul class="foot-orgs">
                                for org in data.orgs.iter() {
                                    <li>
                                        <a class="u" href=(org.link.href.as_str())>(org.link.name.as_str())</a>
                                        if !org.projects.is_empty() {
                                            <ul class="foot-sub">
                                                for p in org.projects.iter() {
                                                    <li><a href=(p.href.as_str())>(p.name.as_str())</a></li>
                                                }
                                                if let Some(text) = more(org.projects.len(), org.projects_total) {
                                                    <li><a class="foot-more" href=(org.link.href.as_str())>(text)</a></li>
                                                }
                                            </ul>
                                        }
                                    </li>
                                }
                            </ul>
                        }
                    </section>
                    link_column(id: "foot-boards", title: "Boards", links: &data.boards, none: "No boards yet.", total: data.boards_total, more_href: data.boards_href)
                    if !data.loose.is_empty() {
                        link_column(id: "foot-loose", title: "Projects without organization", links: &data.loose, none: "", total: data.loose_total, more_href: data.loose_href)
                    }
                    <section class="foot-col" aria-labelledby="foot-settings">
                        <h2 id="foot-settings" class="label">"Settings"</h2>
                        <ul>
                            <li><a class="u" href=(data.settings_href.as_str())>"Global settings"</a></li>
                        </ul>
                        <p class="foot-note">"Editor, pause gap and the defaults of new work."</p>
                    </section>
                    <section class="foot-col" aria-labelledby="foot-about">
                        <h2 id="foot-about" class="label">"About"</h2>
                        <ul>
                            <li><a class="u" href=(GITHUB)>"GitHub"</a></li>
                        </ul>
                        <p class="foot-note">"iter " (data.version)</p>
                    </section>
                </div>
            </div>
        </footer>
    })
}
