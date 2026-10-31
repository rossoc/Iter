//! The Wiki tab of the task page: what agents have changed in the project's
//! wiki and nobody has accepted (a diff, with Accept and Throw away), and the
//! notes still waiting to be ingested (with Ingest now).
//!
//! The reading and the git work are `commands::ingest`'s; this is the page
//! and the three routes (`POST /task/{id}/wiki/{accept|revert|ingest}`), each
//! of which goes back to the tab.

use super::task_agent::{Doc, files_block};
use super::ui::button::post_button;
use super::ui::empty_line::empty_line;
use super::ui::labelled::labelled_section;
use super::ui::notice::notice;
use super::url::{WikiAction, task_url, wiki_action_url, wiki_tab_url};
use super::{Id, open_db};
use crate::commands::ingest::{self, Review};
use crate::db::{Db, Table};
use crate::models::{Project, Task};
use crate::wiki::ingest::Mode;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        error::see_other,
        path_param,
        response::Response,
        route,
    },
    view::{View, component, view},
};

path_param!(action: WikiAction, error = bad_request);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(act)
}

/// One line of a diff and what it is, for the colour (the first character
/// says the same in text).
pub struct DiffLine {
    pub class: &'static str,
    pub text: String,
}

pub struct FileChange {
    pub path: String,
    pub kind: &'static str,
    pub lines: Vec<DiffLine>,
}

/// What the tab shows.
pub enum WikiTab {
    /// The project has nowhere to keep a wiki, or it could not be read.
    Unavailable(String),
    Ready {
        task_id: i64,
        review: Box<Review>,
        files: Vec<FileChange>,
    },
}

fn diff_lines(diff: &str) -> Vec<DiffLine> {
    // the header is the first lines, before the first hunk: after it a line
    // that starts with `--` is a removed line, not a file name
    let mut in_header = true;
    diff.lines()
        .enumerate()
        .map(|(i, l)| {
            if l.starts_with("@@") {
                in_header = false;
            }
            let class = match l.as_bytes().first() {
                _ if in_header
                    && i < 6
                    && ["---", "+++", "diff ", "index ", "new file", "deleted file"]
                        .iter()
                        .any(|p| l.starts_with(p)) =>
                {
                    "meta"
                }
                Some(b'+') => "add",
                Some(b'-') => "del",
                Some(b'@') => "hunk",
                _ => "ctx",
            };
            DiffLine {
                class,
                text: l.to_string(),
            }
        })
        .collect()
}

pub fn load(db: &Db, project: &Project, task: &Task) -> WikiTab {
    match ingest::review(db, project) {
        Ok(review) => {
            let files = review
                .changes
                .iter()
                .map(|c| FileChange {
                    path: c.path.clone(),
                    kind: c.kind,
                    lines: diff_lines(&c.diff),
                })
                .collect();
            WikiTab::Ready {
                task_id: task.id(),
                review: Box::new(review),
                files,
            }
        }
        Err(e) => WikiTab::Unavailable(e.to_string()),
    }
}

fn mode_text(mode: Mode) -> &'static str {
    match mode {
        Mode::Manual => "only when you ask",
        Mode::OnTaskStart => "when a task starts",
        Mode::OnTaskDone => "when a task is marked done",
    }
}

#[component]
async fn changes_block(task_id: i64, review: &Review, files: &[FileChange]) -> Result<impl View> {
    Ok(view! {
        labelled_section(id: "pending-title", label: "Pending changes", count: Some(files.len()), noun: "files",
            if !review.in_git {
                empty_line("The notes folder " <code>(review.notes_dir.display().to_string())</code> " is not in a git repository, so there is nothing to review the agents' changes against. Run " <code>"git init"</code> " in it.")
            } else if files.is_empty() {
                empty_line("Nothing pending: the wiki is as it was last committed.")
            } else {
                if review.busy {
                    notice("An agent is writing in the wiki now. Accept or throw away once it has finished.")
                }
                <div class="step-actions">
                    post_button(action: wiki_action_url(task_id, WikiAction::Accept), fields: vec![], (format!("Accept {} files", files.len())))
                    post_button(action: wiki_action_url(task_id, WikiAction::Revert), fields: vec![], (format!("Throw away {} files", files.len())))
                </div>
                <p class="empty-line">"Accept commits them in the notes' repository. Throw away removes every uncommitted change under " <code>"wiki/"</code> ", yours too."</p>
                <div class="files diffs">
                    for file in files.iter() {
                        <details open="">
                            <summary><span class="step-tag">(file.kind)</span> " " <code>(file.path.as_str())</code></summary>
                            <pre class="diff" tabindex="0">
                                for line in file.lines.iter() {
                                    <span class=(line.class)>(line.text.as_str())</span>"\n"
                                }
                            </pre>
                        </details>
                    }
                </div>
            }
        )
    })
}

#[component]
async fn waiting_block(task_id: i64, review: &Review) -> Result<impl View> {
    let waiting = &review.waiting;
    Ok(view! {
        labelled_section(id: "waiting-title", label: "Waiting to be ingested", count: Some(waiting.len()), noun: "notes",
            <p class="empty-line">"Ingest runs " (mode_text(review.mode)) ". Set " <code>"ingest"</code> " in " <code>"wiki/settings.yaml"</code> " of the project or its organization to change it."</p>
            if waiting.is_empty() {
                empty_line("Every note has pages made after it last changed.")
            } else {
                <ul class="waiting">
                    for w in waiting.iter() {
                        <li><code>(w.id.as_str())</code></li>
                    }
                </ul>
            }
            if review.in_git && !review.busy {
                <div class="step-actions">
                    post_button(action: wiki_action_url(task_id, WikiAction::Ingest), fields: vec![], "Ingest now, with this task's sketches")
                </div>
            }
            if review.busy {
                notice("An agent is writing in the wiki now.")
            }
        )
    })
}

/// The tab's body.
#[component]
pub async fn wiki_tab(tab: &WikiTab, sketches: &[Doc]) -> Result<impl View> {
    Ok(view! {
        if let WikiTab::Unavailable(why) = tab {
            empty_line((why.as_str()))
        } else if let WikiTab::Ready { task_id, review, files } = tab {
            changes_block(task_id: *task_id, review: review, files: files)
            waiting_block(task_id: *task_id, review: review)
        }
        files_block(label: "Sketches", noun: "sketches", docs: sketches)
    })
}

/// `POST /task/{id}/wiki/{accept|revert|ingest}`.
#[route(POST "/task/{id}/wiki/{action}")]
async fn act(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let action = *path_param::<Action>(cx)?;
    let db = open_db()?;
    let (task, project) = super::load::task_with_project(&db, id)?;
    let done = match action {
        WikiAction::Accept => ingest::settle(&db, &project, true).map(|_| ()),
        WikiAction::Revert => ingest::settle(&db, &project, false).map(|_| ()),
        WikiAction::Ingest => ingest::ingest_in_background(&db, &project, &task),
    };
    done.map_err(super::errors::refused)?;
    Err(see_other(wiki_tab_url(&task_url(id))).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diff_is_split_into_lines_that_know_what_they_are() {
        let lines = diff_lines("--- /dev/null\n+++ b/wiki/a.md\n@@ -1 +1 @@\n-old\n+new\n same\n");
        let classes: Vec<&str> = lines.iter().map(|l| l.class).collect();
        assert_eq!(classes, ["meta", "meta", "hunk", "del", "add", "ctx"]);
        assert_eq!(lines[4].text, "+new");
    }
}
