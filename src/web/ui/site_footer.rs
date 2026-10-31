//! The footer of every page: wordmark and a second menu of organizations with
//! their projects, boards, loose projects and settings. Markup and CSS only; data
//! comes from `web/footer.rs`.

use topcoat::{
    Result,
    view::{View, component, view},
};

pub struct FooterLink {
    pub name: String,
    pub href: String,
}

/// An organization link with its projects (compact, muted); `projects` may be the
/// first few of `projects_total`, the rest ending in a link to the organization.
pub struct FooterGroup {
    pub link: FooterLink,
    pub projects: Vec<FooterLink>,
    pub projects_total: usize,
}

/// Everything the footer shows; a cut list ends with a "N more…" link to its `_href`.
pub struct FooterData {
    pub orgs: Vec<FooterGroup>,
    pub boards: Vec<FooterLink>,
    pub boards_total: usize,
    pub boards_href: &'static str,
    pub loose: Vec<FooterLink>,
    pub loose_total: usize,
    pub loose_href: &'static str,
    pub settings_href: String,
    pub version: &'static str,
}

pub const GITHUB: &str = "https://github.com/rossoc/Iter";

/// The link text ending a cut list; `None` when nothing is cut.
fn more(shown: usize, total: usize) -> Option<String> {
    (total > shown).then(|| format!("{} more\u{2026}", total - shown))
}

/// A column's title and links, or `none`; a cut list ends with the link to `more_href`.
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
