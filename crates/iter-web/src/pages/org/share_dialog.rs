//! The Share pop-up of the Report tab and what a share answers with: the data (`ShareForm`,
//! `ShareDialog`, `Outcome`) and the views. The push itself is `org_share.rs`, which renders
//! the host page; this module imports neither it nor the host.

use crate::forms::text;
use crate::pages::org::platform_pick;
use crate::pages::org::platform_pick::platform_lists;
use crate::prelude::*;
use chrono::NaiveDateTime;
use iter_core::db::Db;
use iter_core::error::{IterError, Result as IterResult};
use iter_core::models::Organization;
use iter_core::platform::PlatformItem;
use iter_core::push::{Item, Target};
use iter_core::reporting::parse_day;
use iter_core::reporting::{DateRange, fmt_date};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modality {
    Copy,
    Push,
}

impl Modality {
    pub fn as_str(self) -> &'static str {
        match self {
            Modality::Copy => "copy",
            Modality::Push => "push",
        }
    }
}

impl std::str::FromStr for Modality {
    type Err = String;

    fn from_str(word: &str) -> Result<Self, String> {
        [Modality::Copy, Modality::Push]
            .into_iter()
            .find(|m| m.as_str() == word)
            .ok_or_else(|| format!("unknown way to share '{word}'"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareFormat {
    Worklog,
    Status,
}

impl ShareFormat {
    pub const ALL: [ShareFormat; 2] = [ShareFormat::Worklog, ShareFormat::Status];

    pub fn as_str(self) -> &'static str {
        match self {
            ShareFormat::Worklog => "worklog",
            ShareFormat::Status => "status",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ShareFormat::Worklog => "Worklog",
            ShareFormat::Status => "Status list",
        }
    }
}

impl std::str::FromStr for ShareFormat {
    type Err = String;

    fn from_str(word: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|f| f.as_str() == word)
            .ok_or_else(|| format!("unknown format '{word}'"))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareForm {
    pub from: String,
    pub to: String,
    pub modality: String,
    pub format: String,
    pub organization: String,
    pub activity: String,
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
    pub projects: Vec<PlatformItem>,
    pub activities: Vec<PlatformItem>,
    pub hint: String,
}

impl ShareDialog {
    /// `worklog` and `status` are the texts the Copy button hands over, per [`ShareFormat`].
    pub fn new(
        org: &Organization,
        base: &str,
        range: &DateRange,
        texts: (String, String),
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
            from: range.from.map(fmt_date).unwrap_or_default(),
            to: fmt_date(range.to),
            worklog: texts.0,
            status: texts.1,
            target,
            projects,
            activities,
            hint,
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Error(String),
    Copied(String),
    Preview(Box<Preview>),
    Done(Vec<Item>),
    /// A push already made, read back from the ledger after the redirect (`?pushed=<hash>`): its result lines.
    Recorded(Vec<String>),
}

/// The result of the push with this hash, as the ledger recorded it.
pub fn recorded(db: &Db, id: i64, hash: &str) -> IterResult<Option<Outcome>> {
    Ok(db
        .find_push(id, hash)?
        .map(|row| Outcome::Recorded(row.results.lines().map(String::from).collect())))
}

#[derive(Debug)]
pub struct Preview {
    pub text: String,
    pub warnings: Vec<String>,
    pub conflicts: Vec<String>,
    pub already: bool,
    pub changed: bool,
    pub form: ShareForm,
}

/// Without script both ways show with their button; `org.js` shows the chosen one only and copies in the page.
#[component]
pub async fn share_modal(dialog: &ShareDialog) -> Result<impl View> {
    let formats = ShareFormat::ALL.map(|f| (f.as_str().to_string(), f.label().to_string()));
    Ok(view! {
        modal(id: "share-title", title: "Share this period", close: dialog.close.as_str(),
            <form class="form sharing" method="post" action=(dialog.action.as_str())
                data-worklog=(dialog.worklog.as_str()) data-status=(dialog.status.as_str())>
                <input type="hidden" name="from" value=(dialog.from.as_str())>
                <input type="hidden" name="to" value=(dialog.to.as_str())>
                <fieldset class="share-modes" hidden="">
                    <legend class="sr">"How to share"</legend>
                    <label class="mode"><input type="radio" name="mode" value=(Modality::Copy.as_str()) checked="">(icon(COPY)) "Copy"</label>
                    <label class="mode"><input type="radio" name="mode" value=(Modality::Push.as_str())>(icon(PUSH)) "Push"</label>
                </fieldset>
                <div class="share-set" data-for=(Modality::Copy.as_str())>
                    select(name: "format", label: "Format", options: &formats, current: ShareFormat::Worklog.as_str())
                </div>
                <div class="share-set" data-for=(Modality::Push.as_str())>
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
                    <button type="submit" name="modality" value=(Modality::Copy.as_str()) class="button primary share-go" data-for=(Modality::Copy.as_str())>
                        <span class="idle">(icon(COPY)) "Copy"</span>
                        <span class="done">(icon(CLIPBOARD_CHECK)) "Copied"</span>
                    </button>
                    <button type="submit" name="modality" value=(Modality::Push.as_str()) class="button primary share-go" data-for=(Modality::Push.as_str())>(icon(PUSH)) "Push"</button>
                    <a class="u" href=(dialog.close.as_str())>"Cancel"</a>
                    <span class="sr" id="copy-status" role="status"></span>
                </div>
            </form>
        )
    })
}

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
                <input type="hidden" name="modality" value=(Modality::Push.as_str())>
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

#[component]
pub async fn share_outcome(outcome: &Outcome) -> Result<impl View> {
    Ok(view! {
        <section class="share-result" aria-label="Share result">
            if let Outcome::Error(message) = outcome {
                notice(error: true, id: "share-result", focus: true, (message.as_str()))
            } else if let Outcome::Copied(text) = outcome {
                notice(id: "share-result", focus: true, "Copy this text:")
                <textarea class="control share-text" readonly="" rows="8" aria-label="Text to copy">(text.as_str())</textarea>
            } else if let Outcome::Recorded(lines) = outcome {
                notice(error: lines.iter().any(|l| l.starts_with("FAILED")), id: "share-result", focus: true,
                    (if lines.iter().any(|l| l.starts_with("FAILED")) { "Pushed, but some steps failed." } else { "Pushed." })
                )
                <ul class="share-items">
                    for line in lines.iter() {
                        <li class=(if line.starts_with("FAILED") { "bad" } else { "" })>(line.as_str())</li>
                    }
                </ul>
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

    #[test]
    fn the_words_of_the_form_round_trip() {
        for m in [Modality::Copy, Modality::Push] {
            assert_eq!(m.as_str().parse(), Ok(m));
        }
        for f in ShareFormat::ALL {
            assert_eq!(f.as_str().parse(), Ok(f));
        }
        assert_eq!(
            "mail".parse::<Modality>(),
            Err("unknown way to share 'mail'".into())
        );
        assert_eq!("".parse::<ShareFormat>(), Err("unknown format ''".into()));
    }
}
