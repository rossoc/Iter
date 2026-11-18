//! Suggestions from the organization's time platform for the project and activity fields (settings form, Share pop-up); a platform that cannot answer gives none.

use super::assets::{Asset, json, serve};
use super::{Id, open_db};
use crate::models::Organization;
use crate::platform::{JsonWebhook, PlatformItem};
use crate::push;
use crate::secret::{self, Keychain};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, path_param, query_params, response::Response, route},
    view::{View, component, view},
};

const WAIT_SECONDS: u64 = 5;

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    serve(builder, &[("/platform.js", &PLATFORM_JS)]).route(activities_of)
}

static PLATFORM_JS: Asset = Asset::js(include_str!("platform.js"));

pub fn open(org: &Organization) -> Option<JsonWebhook> {
    let url = org.platform_url.trim();
    if url.is_empty() {
        return None;
    }
    let key = secret::resolve_env(&Keychain, &org.name).ok()?;
    Some(JsonWebhook::new(url, key).ok()?.with_timeout(WAIT_SECONDS))
}

pub fn lists(org: &Organization, project: &str) -> (Vec<PlatformItem>, Vec<PlatformItem>) {
    let Some(platform) = open(org) else {
        return Default::default();
    };
    let project = match project.trim() {
        "" => org.name.as_str(),
        given => given,
    };
    push::choices(&platform, project)
}

pub const LIST_PROJECTS: &str = "platform-projects";
pub const LIST_ACTIVITIES: &str = "platform-activities";

#[component]
pub async fn platform_lists(
    url: &str,
    projects: &[PlatformItem],
    activities: &[PlatformItem],
) -> Result<impl View> {
    Ok(view! {
        <datalist id=(LIST_PROJECTS)>
            for p in projects.iter() {
                <option value=(p.name.as_str())></option>
            }
        </datalist>
        <datalist id=(LIST_ACTIVITIES) data-url=(url)>
            for a in activities.iter() {
                <option value=(a.name.as_str())></option>
            }
        </datalist>
        <script src="/platform.js" defer=""></script>
    })
}

#[query_params(error = bad_request)]
struct ProjectQuery {
    project: Option<String>,
}

#[derive(Deserialize, serde::Serialize)]
struct Activities {
    activities: Vec<String>,
}

/// `POST /org/{id}/platform-activities?project=NAME`: the project's activity names as JSON (empty
/// when the platform cannot say). A POST although it changes nothing: it makes an authenticated
/// call to the platform, which a GET from any site (an `<img>`) could set off.
#[route(POST "/org/{id}/platform-activities")]
async fn activities_of(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<ProjectQuery>(cx)?;
    let db = open_db()?;
    let names = match db.get::<Organization>(id)? {
        Some(org) => lists(&org, query.project.as_deref().unwrap_or_default())
            .1
            .into_iter()
            .map(|a| a.name)
            .collect(),
        None => Vec::new(),
    };
    let body = serde_json::to_string(&Activities { activities: names }).unwrap_or_default();
    json(cx, body, false)
}
