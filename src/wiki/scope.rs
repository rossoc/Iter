//! Which wikis a question sees: turns what the database knows (task, project, organization) into the [`Level`]s the graph is built from.
//! A task sees its own folder, its project's wiki and its organization's, innermost first; an organization may go `down` into its projects' wikis, never a task's folder (private).

use super::graph::{Level, Wiki};
use super::ontology::{LevelKind, Ontology};
use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::git::slugify;
use crate::models::{Organization, Project, Task};
use crate::notes;
use crate::utils::resolve::{current_session_task, resolve_task};
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

fn org_of(db: &Db, project: &Project) -> Result<Option<Organization>> {
    match project.organization_id {
        Some(id) => Ok(db.get::<Organization>(id)?),
        None => Ok(None),
    }
}

fn ask(db: &Db, spec: &Spec) -> Result<Asked> {
    if let Some(reference) = &spec.task {
        let (project, task) = resolve_task(db, reference)?;
        let org = org_of(db, &project)?;
        return Ok(Asked::Task(project, task, org));
    }
    if let Some(name) = &spec.project {
        let project = db.resolve::<Project>(name)?;
        let org = org_of(db, &project)?;
        return Ok(Asked::Project(project, org));
    }
    if let Some(name) = &spec.org {
        return Ok(Asked::Organization(db.resolve::<Organization>(name)?));
    }
    let (project, task) = current_session_task(db).map_err(|e| match e {
        IterError::NotInTmux => IterError::CommandFailed(
            "not inside a tmux session: say which wiki with --task, --project or --org".into(),
        ),
        other => other,
    })?;
    let org = org_of(db, &project)?;
    Ok(Asked::Task(project, task, org))
}

fn slug(name: &str, kind: &str, id: i64) -> String {
    match slugify(name) {
        s if s.is_empty() => format!("{kind}-{id}"),
        s => s,
    }
}

struct Chain {
    levels: Vec<Level>,
}

impl Chain {
    fn push(&mut self, kind: LevelKind, label: &str, title: &str, dir: Option<PathBuf>) {
        let Some(dir) = dir else { return };
        let parent = self.levels.len().checked_sub(1);
        self.levels.push(Level {
            kind,
            label: label.to_string(),
            title: title.to_string(),
            dir,
            parent,
        });
    }

    fn innermost_first(mut self) -> Vec<Level> {
        let n = self.levels.len();
        self.levels.reverse();
        for level in &mut self.levels {
            level.parent = level.parent.map(|p| n - 1 - p);
        }
        self.levels
    }
}

/// The levels `spec` sees, innermost first (index 0 is where ids are written from); an error if none has a folder.
pub fn levels(db: &Db, spec: &Spec) -> Result<Vec<Level>> {
    let asked = ask(db, spec)?;
    let mut chain = Chain { levels: Vec::new() };
    let mut siblings = Vec::new();
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
        match &org_label {
            Some(o) => format!("{o}.{own}"),
            None => own,
        }
    };
    if let Some(p) = project {
        let label = project_label(p);
        let dir = notes::project_dir(p, &org.cloned());
        if let Some(t) = task {
            let task_level = dir.as_deref().map(|d| notes::task_dir(d, t.id()));
            let task_label = format!("{label}.task-{}", t.id());
            chain.push(LevelKind::Project, &label, &p.name, dir);
            chain.push(LevelKind::Task, &task_label, &t.name, task_level);
        } else {
            chain.push(LevelKind::Project, &label, &p.name, dir);
        }
    } else if spec.down
        && let Some(o) = org
    {
        for p in db.list::<Project>()? {
            if p.organization_id == Some(o.id()) {
                let dir = notes::project_dir(&p, &Some(o.clone()));
                siblings.push((project_label(&p), p.name.clone(), dir));
            }
        }
    }
    let org_index = chain.levels.len().checked_sub(1);
    let mut levels = chain.innermost_first();
    for (label, title, dir) in siblings {
        if let Some(dir) = dir {
            levels.push(Level {
                kind: LevelKind::Project,
                label,
                title,
                dir,
                // the org is the only level when the projects are siblings
                parent: org_index.map(|_| 0),
            });
        }
    }
    if levels.is_empty() {
        return Err(IterError::CommandFailed(
            "no notes folder to keep a wiki in: set `notes_path` on the project or its organization"
                .into(),
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
