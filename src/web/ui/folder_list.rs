//! A folder's subfolders as a list of links, with the way up first. Plain
//! links: each one is a page of the folder browser.

use topcoat::{
    Result,
    view::{View, component, view},
};

/// A folder to go to.
pub struct FolderLink {
    pub name: String,
    pub href: String,
}

/// `up` is the link to the parent folder, if there is one; `dirs` the
/// subfolders; `none` is what is said when there are none (and nothing could
/// not be read).
#[component]
pub async fn folder_list(up: Option<String>, dirs: &[FolderLink], none: &str) -> Result<impl View> {
    Ok(view! {
        <ul class="folder-list">
            if let Some(href) = &up {
                <li class="up"><a href=(href.as_str())>".. " <span class="muted">"(up one folder)"</span></a></li>
            }
            for d in dirs.iter() {
                <li><a href=(d.href.as_str())>(d.name.as_str())</a></li>
            }
        </ul>
        if dirs.is_empty() {
            <p class="folder-none">(none)</p>
        }
    })
}
