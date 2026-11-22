//! `GET /search?q=`: the palette's results as JSON, read from the database and notes folders on each request and ranked by `iter_core::front::search`.

use crate::{assets::json, prelude::*};
use iter_core::{
    db::{Db, Table},
    front::search::{Entry, Hit, Kind, search},
    models::{Board, Organization, Project, Task},
    notes,
};
use std::{collections::HashMap, path::PathBuf};

const LIMIT: usize = 50;

#[query_params(error = bad_request)]
struct SearchQuery {
    q: Option<String>,
}

type Trail = Vec<(Kind, String)>;

fn entry(kind: Kind, title: &str, above: &Trail, url: String, description: &str) -> Entry {
    Entry {
        kind,
        title: title.to_string(),
        ancestors: above.clone(),
        url,
        description: description.to_string(),
        status: String::new(),
    }
}

/// One entry per note of `dir` (if the owner has a notes folder), under `owner`, the owner's own path.
fn note_entries(dir: Option<PathBuf>, base: &str, owner: &Trail) -> impl Iterator<Item = Entry> {
    let notes = dir.map(|dir| notes::list(&dir)).unwrap_or_default();
    notes
        .into_iter()
        .map(|n| entry(Kind::Note, &n.title, owner, note_url(base, n.date), &n.body))
}

fn entries(db: &Db) -> iter_core::error::Result<Vec<Entry>> {
    let orgs = db.list::<Organization>()?;
    let org_of: HashMap<i64, &Organization> = orgs.iter().map(|o| (o.id(), o)).collect();
    let none = Trail::new();
    let mut out: Vec<Entry> = db
        .list::<Board>()?
        .iter()
        .map(|b| {
            entry(
                Kind::Board,
                &b.name,
                &none,
                board_url(b.id()),
                &b.description,
            )
        })
        .collect();
    for org in &orgs {
        let url = org_url(org.id());
        out.push(entry(
            Kind::Org,
            &org.name,
            &none,
            url.clone(),
            &org.description,
        ));
        let own = vec![(Kind::Org, org.name.clone())];
        out.extend(note_entries(notes::org_dir(org), &url, &own));
    }
    let mut path_of: HashMap<i64, Trail> = HashMap::new();
    for project in db.list::<Project>()? {
        let org = project
            .organization_id
            .and_then(|id| org_of.get(&id).copied());
        let above: Trail = org
            .map(|o| (Kind::Org, o.name.clone()))
            .into_iter()
            .collect();
        let url = project_url(project.id());
        out.push(entry(
            Kind::Project,
            &project.name,
            &above,
            url.clone(),
            &project.description,
        ));
        let own = [above, vec![(Kind::Project, project.name.clone())]].concat();
        out.extend(note_entries(
            notes::project_dir(&project, &org.cloned()),
            &url,
            &own,
        ));
        path_of.insert(project.id(), own);
    }
    for task in db.list::<Task>()? {
        if let Some(above) = path_of.get(&task.project_id) {
            out.push(Entry {
                status: task.status.as_str().to_string(),
                ..entry(
                    Kind::Task,
                    &task.name,
                    above,
                    task_url(task.id()),
                    &task.description,
                )
            });
        }
    }
    Ok(out)
}

fn to_json(hits: &[Hit]) -> String {
    let rows: Vec<serde_json::Value> = hits
        .iter()
        .map(|hit| {
            let e = hit.entry;
            let path: Vec<&str> = e.ancestors.iter().map(|(_, t)| t.as_str()).collect();
            serde_json::json!({
                "kind": e.kind.word(),
                "title": e.title,
                "path": path.join(" / "),
                "url": e.url,
                "status": e.status,
                "marks": hit.marks,
            })
        })
        .collect();
    serde_json::Value::Array(rows).to_string()
}

#[route(GET "/search")]
pub(super) async fn results(cx: &Cx) -> Result<Response> {
    let query = query_params::<SearchQuery>(cx)?;
    let all = entries(&fresh_db()?)?;
    let hits = search(&all, query.q.as_deref().unwrap_or_default(), LIMIT);
    let body = to_json(&hits);
    json(cx, body, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_completes_the_same_kinds() {
        let words: Vec<String> = Kind::ALL
            .iter()
            .map(|k| format!("\"{}\"", k.word()))
            .collect();
        let list = format!("var KINDS = [{}];", words.join(","));
        assert!(include_str!("../../ui/search.js").contains(&list));
    }

    #[test]
    fn hits_are_json_with_their_strings_escaped() {
        let entries = [Entry {
            kind: Kind::Task,
            title: "Fix \"quotes\"\n\\".into(),
            ancestors: vec![(Kind::Org, "acme".into()), (Kind::Project, "web".into())],
            url: "/task/1".into(),
            description: String::new(),
            status: "wip".into(),
        }];
        let json = to_json(&search(&entries, "fix", 5));
        let rows: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(rows[0]["title"], "Fix \"quotes\"\n\\");
        assert_eq!(rows[0]["path"], "acme / web");
        assert_eq!(rows[0]["marks"], serde_json::json!([0, 1, 2]));
    }
}
