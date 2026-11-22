//! Sharing the Report tab's period (`POST /org/{id}/share`): copy it as text, or push it to the
//! organization's time platform. The pop-up and its data are `pages/org/share_dialog.rs`.

use crate::{
    pages::org::{
        org::{Loaded, load as load_org, screen},
        org_report::{report_link, share_text},
        share_dialog::{Modality, Outcome, Preview, ShareForm, ShareFormat},
    },
    parts::all::*,
    prelude::*,
};
use chrono::NaiveDateTime;
use iter_core::{
    db::Db,
    error::{IterError, Result as IterResult},
    github::{GhCli, IssueTracker},
    models::Organization,
    platform::{JsonWebhook, TimePlatform},
    push::{self, Target},
    reporting::DateRange,
    secret::{self, Keychain},
};

type Connect<'a> = &'a dyn Fn(&Organization) -> IterResult<Box<dyn TimePlatform>>;

fn connect_real(org: &Organization) -> IterResult<Box<dyn TimePlatform>> {
    let key = secret::resolve_env(&Keychain, &org.name)?;
    Ok(Box::new(JsonWebhook::new(&org.platform_url, key)?))
}

pub fn handle(
    db: &Db,
    id: i64,
    form: &ShareForm,
    now: NaiveDateTime,
    connect: Connect,
    gh: &dyn IssueTracker,
) -> Outcome {
    match answer(db, id, form, now, connect, gh) {
        Ok(outcome) => outcome,
        Err(e) => Outcome::Error(e.to_string()),
    }
}

fn answer(
    db: &Db,
    id: i64,
    form: &ShareForm,
    now: NaiveDateTime,
    connect: Connect,
    gh: &dyn IssueTracker,
) -> IterResult<Outcome> {
    let org = db
        .get::<Organization>(id)?
        .ok_or_else(|| IterError::CommandFailed("no such organization".into()))?;
    let range = form.range(now)?;
    let modality: Modality = form.modality.parse().map_err(IterError::CommandFailed)?;
    match modality {
        Modality::Copy => {
            let projects = db.projects_in::<Organization>(id)?;
            let report = crate::pages::org::org_report::load(
                db,
                &projects,
                Some(&form.from),
                Some(&form.to),
                now,
            )?;
            let format: ShareFormat = form.format.parse().map_err(IterError::CommandFailed)?;
            Ok(Outcome::Copied(share_text(&report, format)))
        }
        Modality::Push => push_step(db, &org, range, form, now, connect, gh),
    }
}

fn push_step(
    db: &Db,
    org: &Organization,
    range: DateRange,
    form: &ShareForm,
    now: NaiveDateTime,
    connect: Connect,
    gh: &dyn IssueTracker,
) -> IterResult<Outcome> {
    if org.platform_url.trim().is_empty() {
        return Err(IterError::CommandFailed(format!(
            "organization '{}' has no time platform URL: set it in its settings",
            org.name
        )));
    }
    let mut target = Target::of(org);
    if !form.organization.is_empty() {
        target.organization = form.organization.clone();
    }
    if !form.activity.is_empty() {
        target.activity = form.activity.clone();
    }
    let platform = connect(org)?;
    let prepared = push::prepare(db, org, range, now, &target, platform.as_ref())?;
    let push::Prepared {
        plan,
        check,
        warnings,
        ..
    } = &prepared;

    // An identical worklog already pushed is previewed (flagged), never pushed twice.
    if form.confirm && form.hash == plan.hash && !check.already_pushed {
        let ids = prepared.resolved()?;
        let items = push::execute(db, org, plan, check, ids, platform.as_ref(), gh, now)?;
        return Ok(Outcome::Done(items));
    }
    let mut next = form.clone();
    next.hash = plan.hash.clone();
    next.organization = target.organization;
    next.activity = target.activity;
    next.confirm = false;
    Ok(Outcome::Preview(Box::new(Preview {
        text: plan.preview(),
        warnings: warnings.clone(),
        conflicts: check.conflict_lines(),
        already: check.already_pushed,
        // asked to confirm a worklog that is no longer the one previewed
        changed: form.confirm,
        form: next,
    })))
}

/// Where a confirmed push goes on to (303): the report, naming the ledger row the push wrote.
/// None for anything else, and for a push the platform took nothing of (no row to show).
fn pushed_url(db: &Db, id: i64, form: &ShareForm, outcome: &Outcome) -> IterResult<Option<String>> {
    if !matches!(outcome, Outcome::Done(_)) || db.find_push(id, &form.hash)?.is_none() {
        return Ok(None);
    }
    let report = report_link(&org_url(id), Some(&form.from), Some(&form.to));
    Ok(Some(page_url(&report, &[("pushed", Some(&form.hash))])))
}

#[page(POST "/org/{id}/share")]
pub(super) async fn share(cx: &Cx, Form(pairs): Form<Vec<(String, String)>>) -> Result<impl View> {
    let id = path_id(cx)?;
    let form = ShareForm::parse(&pairs);
    let db = fresh_db()?;
    let now = now();
    let outcome = handle(&db, id, &form, now, &connect_real, &GhCli::system());
    // A push is answered with a redirect to the report, so a refresh repeats a GET, not the push.
    if let Some(url) = pushed_url(&db, id, &form, &outcome)? {
        return Err(see_other(url).into());
    }
    // the Report tab of the period asked for, or the default when it was bad
    let valid = form.range(now).is_ok();
    let (from, to) = match valid {
        true => (Some(form.from.as_str()), Some(form.to.as_str())),
        false => (None, None),
    };
    let page: Loaded = load_org(&db, id, Section::Report, from, to, None, now)?;
    Ok(view! {
        screen(page: &page, section: Section::Report, today: now.date(), share: Some(&outcome))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::org::share_dialog::recorded;
    use iter_core::fixtures::{self, dt};
    use iter_core::models::{Settings, Task, TaskStatus};
    use iter_core::platform::fake::FakePlatform;
    use iter_core::secret::fake::MemoryKeys;
    use iter_core::secret::{KeyStore, Secret};

    use iter_core::github::fake::Inert as NoGh;

    fn now() -> NaiveDateTime {
        dt("2026-09-04 18:00")
    }

    fn setup(url: &str) -> (Db, i64) {
        let db = fixtures::db();
        let mut org = Organization::template(&Settings::default());
        org.name = "acme".into();
        org.platform_url = url.into();
        let id = db.insert(&org).unwrap();
        let mut p = fixtures::project("web");
        p.organization_id = Some(id);
        let pid = db.insert(&p).unwrap();
        let task = Task {
            status: TaskStatus::Wip,
            ..fixtures::task(pid, "Fix")
        };
        let tid = db.insert(&task).unwrap();
        fixtures::add_session(
            &db,
            tid,
            dt("2026-09-04 09:00"),
            Some(dt("2026-09-04 10:00")),
        );
        (db, id)
    }

    fn form(modality: &str) -> ShareForm {
        ShareForm {
            from: "2026-09-04".into(),
            to: "2026-09-04".into(),
            modality: modality.into(),
            format: "worklog".into(),
            ..Default::default()
        }
    }

    fn fake(_: &Organization) -> IterResult<Box<dyn TimePlatform>> {
        Ok(Box::new(FakePlatform::default()))
    }

    #[test]
    fn bad_dates_are_refused_not_replaced() {
        let (db, id) = setup("http://x/log");
        for (from, to) in [("nope", "2026-09-04"), ("2026-09-05", "2026-09-04")] {
            let f = ShareForm {
                from: from.into(),
                to: to.into(),
                ..form("copy")
            };
            assert!(
                matches!(handle(&db, id, &f, now(), &fake, &NoGh), Outcome::Error(_)),
                "{from}"
            );
        }
    }

    #[test]
    fn a_copy_without_javascript_returns_the_text() {
        let (db, id) = setup("");
        let Outcome::Copied(text) = handle(&db, id, &form("copy"), now(), &fake, &NoGh) else {
            panic!("copied")
        };
        assert_eq!(text, "Worked on Fix\n");
        let status = ShareForm {
            format: "status".into(),
            ..form("copy")
        };
        let Outcome::Copied(text) = handle(&db, id, &status, now(), &fake, &NoGh) else {
            panic!("copied")
        };
        assert_eq!(text, "web:\n- Fix: work in progress");
    }

    #[test]
    fn a_push_needs_a_platform_url() {
        let (db, id) = setup("");
        let Outcome::Error(e) = handle(&db, id, &form("push"), now(), &fake, &NoGh) else {
            panic!("error")
        };
        assert!(e.contains("no time platform URL"), "{e}");
    }

    #[test]
    fn a_push_needs_a_key() {
        let (db, id) = setup("http://x/log");
        let store = MemoryKeys::default();
        let no_key = |org: &Organization| -> IterResult<Box<dyn TimePlatform>> {
            secret::resolve(&store, &org.name, None)?;
            fake(org)
        };
        let Outcome::Error(e) = handle(&db, id, &form("push"), now(), &no_key, &NoGh) else {
            panic!("error")
        };
        assert!(e.contains("no API key"), "{e}");
        store.set("acme", &Secret::new("k").unwrap()).unwrap();
        assert!(matches!(
            handle(&db, id, &form("push"), now(), &no_key, &NoGh),
            Outcome::Preview(_)
        ));
    }

    #[test]
    fn a_push_previews_first_and_pushes_on_the_confirming_request() {
        let (db, id) = setup("http://x/log");
        let Outcome::Preview(p) = handle(&db, id, &form("push"), now(), &fake, &NoGh) else {
            panic!("preview")
        };
        assert!(p.text.contains("Period: 2026-09-04"));
        assert!(!p.form.confirm && !p.form.hash.is_empty() && !p.changed);

        // a stale fingerprint is previewed again, not pushed
        let stale = ShareForm {
            confirm: true,
            hash: "old".into(),
            ..p.form.clone()
        };
        let Outcome::Preview(again) = handle(&db, id, &stale, now(), &fake, &NoGh) else {
            panic!("preview")
        };
        assert!(again.changed);

        let confirm = ShareForm {
            confirm: true,
            ..p.form.clone()
        };
        let Outcome::Done(items) = handle(&db, id, &confirm, now(), &fake, &NoGh) else {
            panic!("done")
        };
        assert!(items[0].line().starts_with("ok: platform"));

        // the identical worklog is now flagged on its preview
        let Outcome::Preview(second) = handle(&db, id, &form("push"), now(), &fake, &NoGh) else {
            panic!("preview")
        };
        assert!(second.already && second.conflicts.is_empty());
    }

    /// A refresh of the confirming POST must not push again: one ledger row, and the second
    /// answer is the flagged preview. The first is a redirect to the report naming the row.
    #[test]
    fn a_confirmed_push_is_redirected_and_cannot_be_repeated() {
        let (db, id) = setup("http://x/log");
        let Outcome::Preview(p) = handle(&db, id, &form("push"), now(), &fake, &NoGh) else {
            panic!("preview")
        };
        let confirm = ShareForm {
            confirm: true,
            ..p.form.clone()
        };
        let first = handle(&db, id, &confirm, now(), &fake, &NoGh);
        let url = pushed_url(&db, id, &confirm, &first).unwrap().unwrap();
        assert_eq!(
            url,
            format!(
                "/org/{id}?tab=report&from=2026-09-04&to=2026-09-04&pushed={}",
                confirm.hash
            )
        );

        let Outcome::Preview(again) = handle(&db, id, &confirm, now(), &fake, &NoGh) else {
            panic!("a second confirm is previewed, not pushed")
        };
        assert!(again.already);
        assert_eq!(db.list::<iter_core::models::Push>().unwrap().len(), 1);
        assert!(
            pushed_url(&db, id, &confirm, &Outcome::Error("x".into()))
                .unwrap()
                .is_none()
        );

        let Some(Outcome::Recorded(lines)) = recorded(&db, id, &confirm.hash).unwrap() else {
            panic!("recorded")
        };
        assert!(lines[0].starts_with("ok: platform"));
        assert!(recorded(&db, id, "nope").unwrap().is_none());
    }

    #[test]
    fn a_day_the_platform_holds_is_red_in_the_preview_and_not_pushed() {
        let (db, id) = setup("http://x/log");
        let taken = |_: &Organization| -> IterResult<Box<dyn TimePlatform>> {
            Ok(Box::new(FakePlatform {
                existing: vec![iter_core::reporting::WorklogEntry {
                    project: "acme".into(),
                    activity: "dev".into(),
                    day: "2026-09-04".into(),
                    time: 3.0,
                    note: "- x".into(),
                }],
                ..Default::default()
            }))
        };
        let Outcome::Preview(p) = handle(&db, id, &form("push"), now(), &taken, &NoGh) else {
            panic!("preview")
        };
        assert_eq!(p.conflicts.len(), 1);
        assert!(p.conflicts[0].starts_with("2026-09-04"));
        let confirm = ShareForm {
            confirm: true,
            ..p.form.clone()
        };
        let Outcome::Done(items) = handle(&db, id, &confirm, now(), &taken, &NoGh) else {
            panic!("done")
        };
        assert!(items.iter().all(|i| i.error.is_some()));
    }
}
