use super::*;
use iter_core::fixtures;

#[test]
fn the_list_counts_the_runs_each_instruction_was_given() {
    // the shipped instructions, as a config folder holds them
    let scope = Scope {
        project: None,
        org: None,
        layers: vec![(
            Source::Global,
            fixtures::shipped_instructions()
                .parent()
                .unwrap()
                .to_path_buf(),
        )],
        home: None,
    };
    let run = |n: u32, instruction: &str| Use {
        task_id: 1,
        task: "app/Fix it".into(),
        run: RunRecord {
            n,
            agent: iter_core::agent::run::Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
            instruction: instruction.into(),
            wants_worktree: false,
            status: iter_core::agent::run::Status::Done,
            started: format!("2026-10-30 10:0{n}:00"),
            ended: None,
            exit_code: Some(0),
            branch: None,
            worktree: None,
            pid: None,
            subagent: false,
        },
    };
    let uses = vec![run(3, "fix"), run(2, "fix"), run(1, "plan")];
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
            iter_core::models::TaskStatus::Queue,
        ))
        .unwrap();
    let (p, t) = (
        db.get::<Project>(project).unwrap().unwrap(),
        db.get::<Task>(task).unwrap().unwrap(),
    );
    let dir = taskdir::of(&db, &p, &t).unwrap();
    let mut run = RunRecord {
        n: 1,
        agent: iter_core::agent::run::Harness::Claude,
        model: "haiku".into(),
        effort: "low".into(),
        instruction: "review".into(),
        wants_worktree: false,
        status: iter_core::agent::run::Status::Done,
        started: "2026-10-30 10:00:00".into(),
        ended: None,
        exit_code: Some(0),
        branch: None,
        worktree: None,
        pid: None,
        subagent: false,
    };
    dir.write_run(&run).unwrap();
    run.n = 2;
    run.instruction = "fix".into();
    dir.write_run(&run).unwrap();
    let all = usage(&db, None);
    assert_eq!(all.len(), 2);
    assert_eq!(usage(&db, Some(project)).len(), 2);
    assert!(usage(&db, Some(project + 1)).is_empty());
}
