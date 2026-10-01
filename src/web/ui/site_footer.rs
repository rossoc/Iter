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

/// An organization: its link and its projects' (compact, muted).
pub struct FooterGroup {
    pub link: FooterLink,
    pub projects: Vec<FooterLink>,
}

/// Everything the footer shows.
pub struct FooterData {
    pub orgs: Vec<FooterGroup>,
    pub boards: Vec<FooterLink>,
    /// Projects that belong to no organization.
    pub loose: Vec<FooterLink>,
    /// The page of the global settings.
    pub settings_href: String,
    pub version: &'static str,
}

/// Where the project lives.
pub const GITHUB: &str = "https://github.com/rossoc/Iter";

/// A column's title and a flat list of links, or `none` when there are no
/// links.
#[component]
async fn link_column(id: &str, title: &str, links: &[FooterLink], none: &str) -> Result<impl View> {
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
                </ul>
            }
        </section>
    })
}

#[component]
pub async fn site_footer(data: &FooterData) -> Result<impl View> {
    Ok(view! {
        <footer class="v2 site-foot">
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
                                            </ul>
                                        }
                                    </li>
                                }
                            </ul>
                        }
                    </section>
                    link_column(id: "foot-boards", title: "Boards", links: &data.boards, none: "No boards yet.")
                    if !data.loose.is_empty() {
                        link_column(id: "foot-loose", title: "Projects without organization", links: &data.loose, none: "")
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
