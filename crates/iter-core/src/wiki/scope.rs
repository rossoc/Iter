//! Which wikis a question sees: turns what the database knows (task, project, organization) into the [`Level`]s the graph is built from.
//! A task sees its own folder, its project's wiki and its organization's, innermost first; an organization may go `down` into its projects' wikis, never a task's folder (private).

use super::graph::{Level, Wiki};
use super::ontology::{LevelKind, Ontology};
use crate::db::{Db, Table};
use crate::error::{IterError, Result, failed};
use crate::locate::{self, current_project, current_task};
use crate::models::{Organization, Project, Task};
use crate::notes;
use crate::text;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Spec {
    pub task: Option<String>,
    pub project: Option<String>,
    pub org: Option<String>,
    pub down: bool,
}

enum Asked {
    Task(Project, Task, Option<Organization>),
    Project(Project, Option<Organization>),
    Organization(Organization),
}

fn ask(db: &Db, spec: &Spec) -> Result<Asked> {
    let task = |(project, task): (Project, Task)| {
        let org = db.org_of(&project)?;
        Ok(Asked::Task(project, task, org))
    };
    let project = |project: Project| {
        let org = db.org_of(&project)?;
        Ok(Asked::Project(project, org))
    };
    if let Some(reference) = &spec.task {
        return task(locate::task(db, &reference.parse()?)?);
    }
    if let Some(name) = &spec.project {
        return project(db.resolve::<Project>(name)?);
    }
    if let Some(name) = &spec.org {
        return Ok(Asked::Organization(db.resolve::<Organization>(name)?));
    }
    // here: the current task's wiki, else (in a project's folder, a wiki ingest) the project's
    match current_task(db) {
        Ok(found) => task(found),
        Err(IterError::NoCurrentTask | IterError::UntrackedTmuxSession(_)) => {
            project(current_project(db).map_err(|e| match e {
                IterError::NoCurrentProject => failed(
                    "no current task or project: say which wiki with --task, --project or --org",
                ),
                other => other,
            })?)
        }
        Err(e) => Err(e),
    }
}

fn slug(name: &str, kind: &str, id: i64) -> String {
    match text::slug(name) {
        s if s.is_empty() => format!("{kind}-{id}"),
        s => s,
    }
}

/// The levels of one scope, outermost first until [`Chain::innermost_first`] turns them round.
struct Chain(Vec<Level>);

impl Chain {
    /// Adds a level; one whose notes folder is unknown is left out.
    fn push(&mut self, kind: LevelKind, label: &str, title: &str, dir: Option<PathBuf>) {
        let Some(dir) = dir else { return };
        self.0.push(Level {
            kind,
            label: label.to_string(),
            title: title.to_string(),
            dir,
            parent: None,
        });
    }

    /// Innermost first, each level's parent the next one.
    fn innermost_first(mut self) -> Vec<Level> {
        self.0.reverse();
        let n = self.0.len();
        for (i, level) in self.0.iter_mut().enumerate() {
            level.parent = (i + 1 < n).then_some(i + 1);
        }
        self.0
    }
}

/// The levels `spec` sees, innermost first (index 0 is where ids are written from); an error if none has a folder.
pub fn levels(db: &Db, spec: &Spec) -> Result<Vec<Level>> {
    let asked = ask(db, spec)?;
    let (mut chain, mut siblings) = (Chain(Vec::new()), Chain(Vec::new()));
    let (org, project, task) = match &asked {
        Asked::Task(p, t, o) => (o.as_ref(), Some(p), Some(t)),
        Asked::Project(p, o) => (o.as_ref(), Some(p), None),
        Asked::Organization(o) => (Some(o), None, None),
    };
    let org_label = org.map(|o| slug(&o.name, "org", o.id()));
    if let (Some(o), Some(label)) = (org, &org_label) {
        chain.push(LevelKind::Organization, label, &o.name, notes::org_dir(o));
    }
    let project_label = |p: &Project| {
        let own = slug(&p.name, "project", p.id());
        org_label
            .as_ref()
            .map_or(own.clone(), |o| format!("{o}.{own}"))
    };
    if let Some(p) = project {
        let label = project_label(p);
        let dir = notes::project_dir(p, &org.cloned());
        let task_level = task.map(|t| (t, dir.as_deref().map(|d| notes::task_dir(d, t.id()))));
        chain.push(LevelKind::Project, &label, &p.name, dir);
        if let Some((t, task_dir)) = task_level {
            let task_label = format!("{label}.task-{}", t.id());
            chain.push(LevelKind::Task, &task_label, &t.name, task_dir);
        }
    } else if spec.down
        && let Some(o) = org
    {
        for p in db.list::<Project>()? {
            if p.organization_id == Some(o.id()) {
                let dir = notes::project_dir(&p, &Some(o.clone()));
                siblings.push(LevelKind::Project, &project_label(&p), &p.name, dir);
            }
        }
    }
    // the projects are siblings under the organization, the only level of the chain
    let has_org = !chain.0.is_empty();
    let mut levels = chain.innermost_first();
    levels.extend(siblings.0.into_iter().map(|level| Level {
        parent: has_org.then_some(0),
        ..level
    }));
    if levels.is_empty() {
        return Err(failed(
            "no notes folder to keep a wiki in: set `notes_path` on the project or its organization",
        ));
    }
    Ok(levels)
}

/// The ontology extended by the levels' `wiki/ontology.ttl` (outermost first, so the nearest has the last word).
pub fn ontology(levels: &[Level]) -> Result<Ontology> {
    let extension: String = levels
        .iter()
        .rev()
        .filter(|l| l.kind != LevelKind::Task)
        .filter_map(|l| std::fs::read_to_string(l.dir.join("wiki/ontology.ttl")).ok())
        .collect::<Vec<_>>()
        .join("\n");
    Ontology::load((!extension.is_empty()).then_some(extension.as_str()))
}

pub fn wiki(db: &Db, spec: &Spec) -> Result<Wiki> {
    let levels = levels(db, spec)?;
    let ontology = ontology(&levels)?;
    Wiki::build(levels, ontology)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn world() -> (Db, i64, i64) {
        let db = fixtures::db();
        let org = db
            .insert(&Organization {
                name: "Acme Corp".into(),
                notes_path: "/notes/acme".into(),
                ..Organization::default()
            })
            .expect("org");
        let mut app = fixtures::project("App");
        app.organization_id = Some(org);
        let app = db.insert(&app).expect("project");
        let mut loose = fixtures::project("Loose");
        loose.notes_path = "/notes/loose".into();
        db.insert(&loose).expect("loose");
        (db, org, app)
    }

    fn summary(levels: &[Level]) -> Vec<(String, String, Option<usize>)> {
        levels
            .iter()
            .map(|l| (l.label.clone(), l.dir.display().to_string(), l.parent))
            .collect()
    }

    #[test]
    fn a_task_sees_itself_its_project_and_its_organization() {
        let (db, _, app) = world();
        let task = fixtures::add_task(&db, app, "Fix it");
        let spec = Spec {
            task: Some("App/Fix it".into()),
            ..Spec::default()
        };
        let levels = levels(&db, &spec).expect("levels");
        assert_eq!(
            summary(&levels),
            [
                (
                    "acme-corp.app.task-".to_string() + &task.to_string(),
                    format!("/notes/acme/app/tasks/{task}"),
                    Some(1)
                ),
                ("acme-corp.app".into(), "/notes/acme/app".into(), Some(2)),
                ("acme-corp".into(), "/notes/acme".into(), None),
            ]
        );
        assert_eq!(levels[0].kind, LevelKind::Task);
    }

    #[test]
    fn a_project_without_an_organization_is_its_own_scope() {
        let (db, _, _) = world();
        let spec = Spec {
            project: Some("Loose".into()),
            ..Spec::default()
        };
        assert_eq!(
            summary(&levels(&db, &spec).expect("levels")),
            [("loose".to_string(), "/notes/loose".to_string(), None)]
        );
    }

    #[test]
    fn an_organization_goes_down_into_its_projects_only_when_asked() {
        let (db, _, _) = world();
        let mut spec = Spec {
            org: Some("Acme Corp".into()),
            ..Spec::default()
        };
        assert_eq!(levels(&db, &spec).expect("levels").len(), 1);
        spec.down = true;
        let down = levels(&db, &spec).expect("levels");
        assert_eq!(
            summary(&down),
            [
                ("acme-corp".to_string(), "/notes/acme".to_string(), None),
                ("acme-corp.app".into(), "/notes/acme/app".into(), Some(0)),
            ]
        );
    }

    #[test]
    fn nowhere_to_keep_notes_is_an_error() {
        let (db, _, _) = world();
        let spec = Spec {
            project: Some("App".into()),
            ..Spec::default()
        };
        assert!(levels(&db, &spec).is_ok());
        db.insert(&fixtures::project("Bare")).expect("bare");
        let spec = Spec {
            project: Some("Bare".into()),
            ..Spec::default()
        };
        assert!(levels(&db, &spec).is_err());
    }
}
