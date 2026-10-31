//! Sharing the Report tab's period (`POST /org/{id}/share`): copy it as text,
//! or push it to the organization's time platform.
//!
//! Pushing takes two requests. The first answers with a preview that names
//! the period, marks an open session, and says what the platform already
//! holds; its Push button is the second, which carries the preview's
//! fingerprint, so a worklog that changed in between is previewed again
//! rather than pushed unseen. The page that comes back is the Report tab of
//! the same period, with the answer above the report.

use super::forms::text;
use super::org::{Loaded, load as load_org, screen};
use super::platform_pick::{self, platform_lists};
use super::sections::Section;
use super::ui::field::{field, select};
use super::ui::modal::modal;
use super::ui::notice::notice;
use super::ui::{CLIPBOARD_CHECK, COPY, PUSH, icon};
use super::{Id, now, open_db};
use crate::db::Db;
use crate::error::{IterError, Result as IterResult};
use crate::models::Organization;
use crate::platform::{JsonWebhook, Named, TimePlatform};
use crate::github::{GhCli, IssueTracker};
use crate::push::{self, Item, Target};
use crate::reporting::{DateRange, fmt_date, worklog_text};
use crate::secret::{self, Keychain};
use crate::utils::report::parse_day;
use chrono::NaiveDateTime;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(share)
}

/// What the Share form submits. `from`/`to` are the period the Report tab
/// shows (an empty `from` is all time).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareForm {
    pub from: String,
    pub to: String,
    /// `copy` or `push`
    pub modality: String,
    /// `worklog` or `status` (only a copy has a choice)
    pub format: String,
    pub organization: String,
    pub activity: String,
    /// The fingerprint of the previewed worklog, and whether this is the
    /// confirming request.
    pub hash: String,
    pub confirm: bool,
}

impl ShareForm {
    pub fn parse(pairs: &[(String, String)]) -> ShareForm {
        let get = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| text(v).trim().to_string())
                .unwrap_or_default()
        };
        ShareForm {
            from: get("from"),
            to: get("to"),
            modality: get("modality"),
            format: get("format"),
            organization: get("organization"),
            activity: get("activity"),
            hash: get("hash"),
            confirm: !get("confirm").is_empty(),
        }
    }

    /// The period, strictly: unlike the Report tab's own picker, a bad date
    /// is an error and never quietly replaced by another period.
    pub fn range(&self, now: NaiveDateTime) -> IterResult<DateRange> {
        let from = match self.from.as_str() {
            "" => None,
            v => Some(parse_day(v)?),
        };
        let to = match self.to.as_str() {
            "" => now.date(),
            v => parse_day(v)?,
        };
        if let Some(from) = from
            && from > to
        {
            return Err(IterError::InvalidDateRange {
                from: fmt_date(from),
                to: fmt_date(to),
            });
        }
        Ok(DateRange { from, to })
    }
}

/// The Share pop-up over the Report tab (`?share=1`): where the form posts,
/// what a copy puts on the clipboard, and what the platform suggests.
#[derive(Debug)]
pub struct ShareDialog {
    pub close: String,
    pub action: String,
    pub activities_url: String,
    pub from: String,
    pub to: String,
    pub worklog: String,
    pub status: String,
    pub target: Target,
    pub projects: Vec<Named>,
    pub activities: Vec<Named>,
    /// Why a push is not possible yet, if it is not.
    pub hint: String,
}

impl ShareDialog {
    pub fn new(
        org: &Organization,
        base: &str,
        report: &super::org_report::Report,
        close: String,
    ) -> Self {
        let target = Target::of(org);
        let (projects, activities) = platform_pick::lists(org, &target.organization);
        let hint = match (org.platform_url.trim().is_empty(), platform_pick::open(org)) {
            (true, _) => {
                "To push, set the organization's time platform URL in its settings.".to_string()
            }
            (false, None) => format!(
                "To push, set the API key with `iter org key {}` (or ITER_PLATFORM_KEY).",
                org.name
            ),
            (false, Some(_)) => String::new(),
        };
        ShareDialog {
            close,
            action: format!("{base}/share"),
            activities_url: format!("{base}/platform-activities"),
            from: report.range.from.map(fmt_date).unwrap_or_default(),
            to: fmt_date(report.range.to),
            worklog: share_text(report, "worklog").unwrap_or_default(),
            status: share_text(report, "status").unwrap_or_default(),
            target,
            projects,
            activities,
            hint,
        }
    }
}

/// What the page says in answer to a share.
#[derive(Debug)]
pub enum Outcome {
    Error(String),
    /// The text, for a browser without a clipboard script.
    Copied(String),
    Preview(Box<Preview>),
    Done(Vec<Item>),
}

#[derive(Debug)]
pub struct Preview {
    pub text: String,
    pub warnings: Vec<String>,
    /// The days the platform already has an entry for: shown in red and not
    /// pushed.
    pub conflicts: Vec<String>,
    /// This exact worklog was pushed before.
    pub already: bool,
    pub changed: bool,
    pub form: ShareForm,
}

/// Opens the organization's platform: its URL and the key from the keychain
/// or the environment.
type Connect<'a> = &'a dyn Fn(&Organization) -> IterResult<Box<dyn TimePlatform>>;

fn connect_real(org: &Organization) -> IterResult<Box<dyn TimePlatform>> {
    let key = secret::resolve_env(&Keychain, &org.name)?;
    Ok(Box::new(JsonWebhook::new(&org.platform_url, key)))
}

/// The text a copy puts on the clipboard, for `format`.
pub fn share_text(report: &super::org_report::Report, format: &str) -> Option<String> {
    match format {
        "worklog" => Some(worklog_text(report.projects.iter().flat_map(|p| &p.tasks))),
        "status" => Some(super::org_report::status_list(report)),
        _ => None,
    }
}

/// Answers a share. Every refusal is an [`Outcome::Error`] to show.
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
    match form.modality.as_str() {
        "copy" => {
            let projects = db.projects_in::<Organization>(id)?;
            let report =
                super::org_report::load(db, &projects, Some(&form.from), Some(&form.to), now)?;
            let text = share_text(&report, &form.format).ok_or_else(|| {
                IterError::CommandFailed(format!("unknown format '{}'", form.format))
            })?;
            Ok(Outcome::Copied(text))
        }
        "push" => push_step(db, &org, range, form, now, connect, gh),
        other => Err(IterError::CommandFailed(format!(
            "unknown way to share '{other}'"
        ))),
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
    let plan = push::plan(db, org, range, now, &target)?;
    let platform = connect(org)?;
    let check = push::check(db, org, &plan, platform.as_ref())?;

    if form.confirm && form.hash == plan.hash {
        let items = push::execute(db, org, &plan, &check, platform.as_ref(), gh, now)?;
        return Ok(Outcome::Done(items));
    }
    let warnings = push::unknown_targets(platform.as_ref(), &target);
    let mut next = form.clone();
    next.hash = plan.hash.clone();
    next.organization = target.organization;
    next.activity = target.activity;
    next.confirm = false;
    Ok(Outcome::Preview(Box::new(Preview {
        text: plan.preview(),
        warnings,
        conflicts: check.conflict_lines(),
        already: check.already_pushed,
        // asked to confirm a worklog that is no longer the one previewed
        changed: form.confirm,
        form: next,
    })))
}

#[page(POST "/org/{id}/share")]
async fn share(cx: &Cx, Form(pairs): Form<Vec<(String, String)>>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let form = ShareForm::parse(&pairs);
    let db = open_db()?;
    let now = now();
    let outcome = handle(&db, id, &form, now, &connect_real, &GhCli::system());
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

/// The pop-up: the way to share (copy or push), its settings, and at the
/// bottom the one button that does it. Without script both ways show with
/// their button; `org.js` shows the chosen one only and copies in the page.
#[component]
pub async fn share_modal(dialog: &ShareDialog) -> Result<impl View> {
    let formats = [
        ("worklog".to_string(), "Worklog".to_string()),
        ("status".to_string(), "Status list".to_string()),
    ];
    Ok(view! {
        modal(id: "share-title", title: "Share this period", close: dialog.close.as_str(),
            <form class="form sharing" method="post" action=(dialog.action.as_str())
                data-worklog=(dialog.worklog.as_str()) data-status=(dialog.status.as_str())>
                <input type="hidden" name="from" value=(dialog.from.as_str())>
                <input type="hidden" name="to" value=(dialog.to.as_str())>
                <fieldset class="share-modes" hidden="">
                    <legend class="sr">"How to share"</legend>
                    <label class="mode"><input type="radio" name="mode" value="copy" checked="">(icon(COPY)) "Copy"</label>
                    <label class="mode"><input type="radio" name="mode" value="push">(icon(PUSH)) "Push"</label>
                </fieldset>
                <div class="share-set" data-for="copy">
                    select(name: "format", label: "Format", options: &formats, current: "worklog")
                </div>
                <div class="share-set" data-for="push">
                    field(name: "organization", label: "Project", value: &dialog.target.organization, identifier: true, list: platform_pick::LIST_PROJECTS,
                        hint: "The platform's project, by name or id.")
                    field(name: "activity", label: "Activity", value: &dialog.target.activity, identifier: true, list: platform_pick::LIST_ACTIVITIES,
                        hint: "The platform's activity, by name or id.")
                    platform_lists(url: &dialog.activities_url, projects: &dialog.projects, activities: &dialog.activities)
                    if !dialog.hint.is_empty() {
                        notice(error: true, (dialog.hint.as_str()))
                    }
                </div>
                <div class="actions">
                    <button type="submit" name="modality" value="copy" class="button primary share-go" data-for="copy">
                        <span class="idle">(icon(COPY)) "Copy"</span>
                        <span class="done">(icon(CLIPBOARD_CHECK)) "Copied"</span>
                    </button>
                    <button type="submit" name="modality" value="push" class="button primary share-go" data-for="push">(icon(PUSH)) "Push"</button>
                    <a class="u" href=(dialog.close.as_str())>"Cancel"</a>
                    <span class="sr" id="copy-status" role="status"></span>
                </div>
            </form>
        )
    })
}

/// The push preview as a pop-up over the report: the period and entries, what
/// is wrong with them (in red), and at the bottom the Push that confirms --
/// or Cancel, back to the report.
#[component]
pub async fn preview_modal(
    #[into] base: String,
    p: &Preview,
    #[into] close: String,
) -> Result<impl View> {
    Ok(view! {
        modal(id: "push-preview-title", title: "Push preview", close: close.as_str(),
            <pre class="share-preview" aria-label="What would be pushed" tabindex="0">(p.text.as_str())</pre>
            for w in p.warnings.iter() {
                notice(error: true, (w.as_str()))
            }
            if p.changed {
                notice(error: true, "The report changed since the preview; check it and push again.")
            }
            if p.already {
                notice("This exact worklog was already pushed.")
            }
            if !p.conflicts.is_empty() {
                <ul class="share-items conflicts" aria-label="Days already on the platform">
                    for line in p.conflicts.iter() {
                        <li class="bad"><span class="sr">"Conflict: "</span>(line.as_str())</li>
                    }
                </ul>
            }
            <form class="form" method="post" action=(format!("{base}/share"))>
                <input type="hidden" name="from" value=(p.form.from.as_str())>
                <input type="hidden" name="to" value=(p.form.to.as_str())>
                <input type="hidden" name="modality" value="push">
                <input type="hidden" name="format" value=(p.form.format.as_str())>
                <input type="hidden" name="organization" value=(p.form.organization.as_str())>
                <input type="hidden" name="activity" value=(p.form.activity.as_str())>
                <input type="hidden" name="hash" value=(p.form.hash.as_str())>
                <input type="hidden" name="confirm" value="1">
                <div class="actions">
                    <button class="button primary share-go" type="submit">
                        (icon(PUSH)) (if p.conflicts.is_empty() { "Push" } else { "Push the other days" })
                    </button>
                    <a class="u" href=(close.as_str())>"Cancel"</a>
                </div>
            </form>
        )
    })
}

/// The answer to a share, above the report.
#[component]
pub async fn share_outcome(outcome: &Outcome) -> Result<impl View> {
    Ok(view! {
        <section class="share-result" aria-label="Share result">
            if let Outcome::Error(message) = outcome {
                notice(error: true, id: "share-result", focus: true, (message.as_str()))
            } else if let Outcome::Copied(text) = outcome {
                notice(id: "share-result", focus: true, "Copy this text:")
                <textarea class="control share-text" readonly="" rows="8" aria-label="Text to copy">(text.as_str())</textarea>
            } else if let Outcome::Done(items) = outcome {
                notice(error: items.iter().any(|i| i.error.is_some()), id: "share-result", focus: true,
                    (if items.iter().any(|i| i.error.is_some()) { "Pushed, but some steps failed." } else { "Pushed." })
                )
                <ul class="share-items">
                    for item in items.iter() {
                        <li class=(if item.error.is_some() { "bad" } else { "" })>(item.line())</li>
                    }
                </ul>
            }
        </section>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::{Session, Settings, Task, TaskStatus};
    use crate::platform::fake::FakePlatform;
    use crate::secret::fake::MemoryKeys;
    use crate::secret::{KeyStore, Secret};

    use crate::github::fake::Inert as NoGh;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

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
        let mut t = Task::template(pid, String::new(), TaskStatus::Wip);
        t.name = "Fix".into();
        let tid = db.insert(&t).unwrap();
        db.insert(&Session {
            id: None,
            task_id: tid,
            start: dt("2026-09-04 09:00"),
            end: Some(dt("2026-09-04 10:00")),
            message: None,
        })
        .unwrap();
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

    #[test]
    fn a_day_the_platform_holds_is_red_in_the_preview_and_not_pushed() {
        let (db, id) = setup("http://x/log");
        let taken = |_: &Organization| -> IterResult<Box<dyn TimePlatform>> {
            Ok(Box::new(FakePlatform {
                existing: vec![crate::reporting::WorklogEntry {
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

    #[test]
    fn the_form_reads_its_fields() {
        let pairs: Vec<(String, String)> = [
            ("from", " 2026-09-01 "),
            ("modality", "push"),
            ("confirm", "1"),
            ("activity", "dev"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let f = ShareForm::parse(&pairs);
        assert_eq!(
            (
                f.from.as_str(),
                f.modality.as_str(),
                f.confirm,
                f.activity.as_str()
            ),
            ("2026-09-01", "push", true, "dev")
        );
    }
}
