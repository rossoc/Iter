use super::*;
use crate::fixtures;

#[test]
fn the_list_counts_the_runs_each_instruction_was_given() {
    let scope = Scope::global();
    let run = |n: u32, instruction: Option<&str>, kind: &str| Use {
        task_id: 1,
        task: "app/Fix it".into(),
        run: RunRecord {
            n,
            agent: crate::agent::run::Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
            kind: kind.into(),
            instruction: instruction.map(String::from),
            status: crate::agent::run::Status::Done,
            started: format!("2026-10-30 10:0{n}:00"),
            ended: None,
            exit_code: Some(0),
            branch: None,
            worktree: None,
            pid: None,
        },
    };
    // a run from before instructions existed counts for its kind
    let uses = vec![
        run(3, Some("fix"), "fix"),
        run(2, None, "fix"),
        run(1, None, "plan"),
    ];
    let rows = rows(&scope, &uses);
    let by = |name: &str| rows.iter().find(|r| r.name == name).unwrap();
    assert_eq!(by("fix").used, 2);
    assert_eq!((by("plan").used, by("develop").used), (1, 0));
    assert!(by("fix").description.starts_with("Reproduce"));
}

#[test]
fn usage_is_found_in_the_runs_of_the_tasks() {
    let notes = fixtures::scratch("usage");
    let db = fixtures::db();
    let mut p = fixtures::project("app");
    p.notes_path = notes.display().to_string();
    let project = db.insert(&p).unwrap();
    let task = db
        .insert(&Task::template(
            project,
            String::new(),
            crate::models::TaskStatus::Queue,
        ))
        .unwrap();
    let (p, t) = (
        db.get::<Project>(project).unwrap().unwrap(),
        db.get::<Task>(task).unwrap().unwrap(),
    );
    let dir = taskdir::of(&db, &p, &t).unwrap();
    let mut run = RunRecord {
        n: 1,
        agent: crate::agent::run::Harness::Claude,
        model: "haiku".into(),
        effort: "low".into(),
        kind: "fix".into(),
        instruction: Some("review".into()),
        status: crate::agent::run::Status::Done,
        started: "2026-10-30 10:00:00".into(),
        ended: None,
        exit_code: Some(0),
        branch: None,
        worktree: None,
        pid: None,
    };
    dir.write_run(&run).unwrap();
    run.n = 2;
    run.instruction = None;
    dir.write_run(&run).unwrap();
    let all = usage(&db, None);
    assert_eq!(all.len(), 2);
    assert_eq!(usage(&db, Some(project)).len(), 2);
    assert!(usage(&db, Some(project + 1)).is_empty());
}
