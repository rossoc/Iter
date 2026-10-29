//! `GET /search?q=`: the palette's results as JSON. The pages are read from
//! the database (and the notes' folders) on each request and ranked by
//! `search.rs`; the palette script only draws what comes back.

use super::open_db;
use super::search::{Entry, Hit, Kind, search};
use super::url::{board_url, note_url, org_url, project_url, task_url};
use crate::db::{Db, Table};
use crate::models::{Board, Organization, Project, Task};
use crate::notes;
use std::collections::HashMap;
use topcoat::{
    Result,
    context::Cx,
    router::{
        HeaderValue, RouterBuilder,
        header::{CACHE_CONTROL, CONTENT_TYPE},
        query_params,
        response::{IntoResponse, Response},
        route,
    },
};

/// How many results one query returns.
const LIMIT: usize = 50;

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(results)
}

#[query_params(error = bad_request)]
struct SearchQuery {
    q: Option<String>,
}

/// Every page of the app the search can reach.
fn entries(db: &Db) -> crate::error::Result<Vec<Entry>> {
    let orgs = db.list::<Organization>()?;
    let projects = db.list::<Project>()?;
    let org_of: HashMap<i64, &Organization> = orgs.iter().map(|o| (o.id(), o)).collect();
    let project_org = |p: &Project| p.organization_id.and_then(|id| org_of.get(&id).copied());
    let ancestors = |p: &Project| {
        let mut path: Vec<(Kind, String)> = Vec::new();
        if let Some(org) = project_org(p) {
            path.push((Kind::Org, org.name.clone()));
        }
        path
    };

    let mut out = Vec::new();
    for board in db.list::<Board>()? {
        out.push(Entry {
            kind: Kind::Board,
            title: board.name.clone(),
            ancestors: Vec::new(),
            url: board_url(board.id()),
            description: board.description.clone(),
            status: String::new(),
        });
    }
    for org in &orgs {
        out.push(Entry {
            kind: Kind::Org,
            title: org.name.clone(),
            ancestors: Vec::new(),
            url: org_url(org.id()),
            description: org.description.clone(),
            status: String::new(),
        });
        let base = org_url(org.id());
        let dir = notes::org_dir(org);
        push_notes(&mut out, dir.as_deref(), &base, (Kind::Org, &org.name), Vec::new());
    }
    let mut by_id: HashMap<i64, Vec<(Kind, String)>> = HashMap::new();
    for project in &projects {
        let mut above = ancestors(project);
        out.push(Entry {
            kind: Kind::Project,
            title: project.name.clone(),
            ancestors: above.clone(),
            url: project_url(project.id()),
            description: project.description.clone(),
            status: String::new(),
        });
        let org = project_org(project).cloned();
        let dir = notes::project_dir(project, &org);
        let base = project_url(project.id());
        push_notes(&mut out, dir.as_deref(), &base, (Kind::Project, &project.name), above.clone());
        above.push((Kind::Project, project.name.clone()));
        by_id.insert(project.id(), above);
    }
    for task in db.list::<Task>()? {
        let Some(above) = by_id.get(&task.project_id) else {
            continue;
        };
        out.push(Entry {
            kind: Kind::Task,
            title: task.name.clone(),
            ancestors: above.clone(),
            url: task_url(task.id()),
            description: task.description.clone(),
            status: task.status.as_str().to_string(),
        });
    }
    Ok(out)
}

/// The notes of the page at `base` (its folder `dir`, if it keeps one).
fn push_notes(
    out: &mut Vec<Entry>,
    dir: Option<&std::path::Path>,
    base: &str,
    owner: (Kind, &str),
    mut above: Vec<(Kind, String)>,
) {
    let Some(dir) = dir else { return };
    above.push((owner.0, owner.1.to_string()));
    for note in notes::list(dir) {
        out.push(Entry {
            kind: Kind::Note,
            title: note.title.clone(),
            ancestors: above.clone(),
            url: note_url(base, note.date),
            description: note.body,
            status: String::new(),
        });
    }
}

/// `text` as a JSON string.
fn json_str(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn to_json(hits: &[Hit]) -> String {
    let mut out = String::from("[");
    for (i, hit) in hits.iter().enumerate() {
        let e = hit.entry;
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"kind\":");
        json_str(e.kind.word(), &mut out);
        out.push_str(",\"title\":");
        json_str(&e.title, &mut out);
        out.push_str(",\"path\":");
        let path: Vec<&str> = e.ancestors.iter().map(|(_, t)| t.as_str()).collect();
        json_str(&path.join(" / "), &mut out);
        out.push_str(",\"url\":");
        json_str(&e.url, &mut out);
        out.push_str(",\"status\":");
        json_str(&e.status, &mut out);
        out.push_str(",\"marks\":[");
        let marks: Vec<String> = hit.marks.iter().map(|m| m.to_string()).collect();
        out.push_str(&marks.join(","));
        out.push_str("]}");
    }
    out.push(']');
    out
}

#[route(GET "/search")]
async fn results(cx: &Cx) -> Result<Response> {
    let query = query_params::<SearchQuery>(cx)?;
    let all = entries(&open_db()?)?;
    let hits = search(&all, query.q.as_deref().unwrap_or_default(), LIMIT);
    let body = to_json(&hits);
    (
        [
            (CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        body,
    )
        .into_response(cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        let mut out = String::new();
        json_str("a \"b\"\n\\\u{1}", &mut out);
        assert_eq!(out, r#""a \"b\"\n\\\u0001""#);
    }
}
