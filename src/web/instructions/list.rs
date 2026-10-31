//! The list page of instructions, and the Instructions tab of a project.

use super::*;

/// The table of instructions, in the look of the tasks table: the first row
/// (`add`) writes a new one.
#[component]
pub async fn instruction_table(
    rows: &[ListRow],
    #[default] add: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        data_table(caption: "Instructions", columns: &["Instruction", "Used"], class: "tasks instructions",
            add: add.map(|href| AddRow::link(href, "New instruction\u{2026}")),
            for r in rows.iter() {
                row(
                    cell(
                        <a class="u" href=(r.href.as_str())>(label_of(&r.name))</a>
                        // its space is kept when there is nothing to say, so rows do not jump
                        <span class="sub">(if r.description.is_empty() { "\u{a0}" } else { r.description.as_str() })</span>
                    )
                    cell(class: "mono", (r.used.to_string()))
                )
            }
        )
    })
}

#[page("/instructions")]
pub(super) async fn list_page(cx: &Cx) -> Result<impl View> {
    let query = query_params::<TabQuery>(cx)?;
    let db = open_db()?;
    let scope = Scope::global();
    let rows = rows(&scope, &usage(&db, None));
    let adding = query.new.as_deref() == Some("instruction");
    Ok(view! {
        list_screen(scope: &scope, rows: &rows, adding: adding)
    })
}

#[component]
async fn list_screen(scope: &Scope, rows: &[ListRow], adding: bool) -> Result<impl View> {
    let add = page_url(&instructions_url(), &[("new", Some("instruction"))]);
    let dialog = adding.then(|| Child::new(view! { new_instruction_modal(scope: scope) }));
    Ok(view! {
        frame(title: &["Instructions"], current: "/instructions", dialog: dialog,
            page_header(title: "Instructions", kicker: Kicker::Eyebrow("Global"))
            <p class="prompts-intro">
                "What an agent is told for a kind of work, in the format of an opencode agent: front matter over the prompt. "
                "These are the ones every project shares; a project or its organization can keep its own of the same name."
            </p>
            instruction_table(rows: rows, add: Some(add))
        )
    })
}
