//! The plan of a task: `tasks/<id>/plan.md`, a markdown file with Goal, How and Checklist sections.
//! A step is `- [ ] {s1} (verify) text`; the file is the only store. Format and rules: `doc/architecture_agent_wiki.md`.

use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// `[ ]`
    Todo,
    /// `[~]`: being worked on
    Doing,
    /// `[x]`
    Done,
}

impl State {
    fn mark(self) -> char {
        match self {
            State::Todo => ' ',
            State::Doing => '~',
            State::Done => 'x',
        }
    }

    fn of(mark: char) -> Option<State> {
        match mark {
            ' ' => Some(State::Todo),
            '~' => Some(State::Doing),
            'x' | 'X' => Some(State::Done),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            State::Todo => "todo",
            State::Doing => "doing",
            State::Done => "done",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Item {
    /// `s3`; empty for a step nobody has numbered yet (see [`normalize`]).
    pub id: String,
    pub state: State,
    /// A step that checks the work rather than doing it: `(verify)`.
    pub verify: bool,
    pub text: String,
    pub detail: String,
    /// Index into [`Plan::phases`]; none above the first `###` heading.
    pub phase: Option<usize>,
    #[serde(skip)]
    pub line: usize,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Plan {
    pub goal: String,
    pub how: String,
    pub items: Vec<Item>,
    pub phases: Vec<String>,
    /// The headings of the other sections, which are left alone.
    pub other: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub doing: usize,
    pub total: usize,
    pub verify_done: usize,
    pub verify_total: usize,
}

impl Plan {
    pub fn progress(&self) -> Progress {
        let mut p = Progress::default();
        for item in &self.items {
            p.total += 1;
            p.done += (item.state == State::Done) as usize;
            p.doing += (item.state == State::Doing) as usize;
            if item.verify {
                p.verify_total += 1;
                p.verify_done += (item.state == State::Done) as usize;
            }
        }
        p
    }

    pub fn phase_progress(&self, phase: usize) -> (usize, usize) {
        let mine = self.items.iter().filter(|i| i.phase == Some(phase));
        let (mut done, mut total) = (0, 0);
        for i in mine {
            total += 1;
            done += (i.state == State::Done) as usize;
        }
        (done, total)
    }

    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
}

enum Section {
    Before,
    Goal,
    How,
    Checklist,
    Other,
}

fn heading(line: &str) -> Option<&str> {
    line.strip_prefix("## ").map(str::trim)
}

fn fence(line: &str) -> bool {
    line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~")
}

/// A step line: `- [ ] {s1} (verify) text`, `*` for `-` too, any indent.
struct Step<'a> {
    state: State,
    id: Option<&'a str>,
    verify: bool,
    text: &'a str,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 16
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn step(line: &str) -> Option<Step<'_>> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("- [")
        .or_else(|| trimmed.strip_prefix("* ["))?;
    let mut chars = rest.chars();
    let state = State::of(chars.next()?)?;
    let rest = chars.as_str().strip_prefix(']')?;
    // a checkbox is followed by a space (or by nothing: an empty step)
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let mut rest = rest.trim_start();
    let mut id = None;
    if let Some(inner) = rest.strip_prefix('{')
        && let Some(end) = inner.find('}')
        && valid_id(&inner[..end])
    {
        id = Some(&inner[..end]);
        rest = inner[end + 1..].trim_start();
    }
    let mut verify = false;
    if rest
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("(verify)"))
    {
        verify = true;
        rest = rest[8..].trim_start();
    }
    let mut text = rest.trim_end();
    // an agent often puts the tag last: `text (verify)` or `text (verify: how)`;
    // only the last parenthesis counts, so `a (verify) then (b)` stays whole
    if !verify && text.ends_with(')') {
        let lower = text.to_ascii_lowercase();
        if let Some(at) = lower.rfind('(')
            && lower[at..].starts_with("(verify")
            && lower[at + 7..].starts_with([')', ':'])
        {
            verify = true;
            text = text[..at].trim_end();
        }
    }
    Some(Step {
        state,
        id,
        verify,
        text,
    })
}

fn body_start(content: &str) -> usize {
    let Some(rest) = content.strip_prefix("---\n") else {
        return 0;
    };
    let mut lines = 1;
    for line in rest.split_inclusive('\n') {
        lines += 1;
        if line.trim_end_matches('\n') == "---" {
            return lines;
        }
    }
    0
}

pub fn parse(content: &str) -> Plan {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let (mut plan, mut section, mut fenced) = (Plan::default(), Section::Before, false);
    let (mut goal, mut how): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    let first = body_start(content);
    for (n, line) in content.lines().enumerate().skip(first) {
        if fence(line) {
            fenced = !fenced;
        } else if !fenced && let Some(name) = heading(line) {
            section = match name.to_lowercase().as_str() {
                "goal" => Section::Goal,
                "how" => Section::How,
                "checklist" => Section::Checklist,
                _ => {
                    plan.other.push(name.to_string());
                    Section::Other
                }
            };
            continue;
        }
        match section {
            Section::Goal => goal.push(line),
            Section::How => how.push(line),
            Section::Checklist if !fenced => {
                if let Some(name) = line.strip_prefix("### ") {
                    plan.phases.push(name.trim().to_string());
                } else if let Some(s) = step(line) {
                    plan.items.push(Item {
                        id: s.id.unwrap_or("").to_string(),
                        state: s.state,
                        verify: s.verify,
                        text: s.text.to_string(),
                        detail: String::new(),
                        phase: plan.phases.len().checked_sub(1),
                        line: n,
                    });
                } else if let Some(last) = plan.items.last_mut() {
                    // indented lines under a step are its detail
                    if line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
                        if !last.detail.is_empty() {
                            last.detail.push('\n');
                        }
                        last.detail.push_str(line.trim());
                    }
                }
            }
            _ => {}
        }
    }
    plan.goal = goal.join("\n").trim().to_string();
    plan.how = how.join("\n").trim().to_string();
    plan
}

fn number(id: &str) -> Option<usize> {
    id.strip_prefix('s')?.parse().ok()
}

/// Rewrites line `n` of `content` with `edit`, every other byte as it was.
fn edit_line(content: &str, n: usize, edit: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(content.len() + 8);
    for (i, line) in content.split_inclusive('\n').enumerate() {
        if i == n {
            let body = line.strip_suffix('\n').unwrap_or(line);
            let newline = &line[body.len()..];
            out.push_str(&edit(body));
            out.push_str(newline);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// `content` with every step numbered: a missing or duplicate id gets the next free `s<n>`.
pub fn normalize(content: &str) -> String {
    let plan = parse(content);
    let mut taken: Vec<String> = Vec::new();
    let mut next = plan
        .items
        .iter()
        .filter_map(|i| number(&i.id))
        .max()
        .unwrap_or(0);
    let mut out = content.to_string();
    for item in &plan.items {
        let fresh = item.id.is_empty() || taken.contains(&item.id);
        if !fresh {
            taken.push(item.id.clone());
            continue;
        }
        next += 1;
        let id = format!("s{next}");
        out = edit_line(&out, item.line, |line| {
            let at = line.find(']').map_or(0, |i| i + 1);
            let (head, tail) = line.split_at(at);
            let tail = if item.id.is_empty() {
                tail.trim_start().to_string()
            } else {
                let t = tail.trim_start();
                t.split_once('}')
                    .map_or(t, |(_, r)| r)
                    .trim_start()
                    .to_string()
            };
            match tail.is_empty() {
                true => format!("{head} {{{id}}}"),
                false => format!("{head} {{{id}}} {tail}"),
            }
        });
        taken.push(id);
    }
    out
}

#[derive(Debug, PartialEq)]
pub enum MarkError {
    NoSuchStep(String, Vec<String>),
}

impl std::fmt::Display for MarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let MarkError::NoSuchStep(id, ids) = self;
        match ids.is_empty() {
            true => write!(f, "no step '{id}': the plan has no steps"),
            false => write!(f, "no step '{id}' (the steps are {})", ids.join(", ")),
        }
    }
}

impl std::error::Error for MarkError {}

/// `content` with step `id` in `state`: only its checkbox changes.
pub fn mark(content: &str, id: &str, state: State) -> Result<String, MarkError> {
    let plan = parse(content);
    let Some(item) = plan.item(id) else {
        let ids = plan.items.iter().map(|i| i.id.clone()).collect();
        return Err(MarkError::NoSuchStep(id.to_string(), ids));
    };
    Ok(edit_line(content, item.line, |line| {
        let open = line.find('[').expect("a step line has a checkbox");
        let mut out = String::from(&line[..open + 1]);
        out.push(state.mark());
        out.push_str(&line[open + 2..]);
        out
    }))
}

/// What a plan lacks that most want; it never stops the plan being saved.
pub fn problems(content: &str) -> Vec<String> {
    let plan = parse(content);
    let mut out = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    if plan.goal.is_empty() {
        out.push("there is no `## Goal`".to_string());
    }
    if plan.items.is_empty() {
        out.push("the `## Checklist` has no steps".to_string());
    }
    for item in &plan.items {
        if !item.id.is_empty() && seen.contains(&item.id.as_str()) {
            out.push(format!("step id '{}' is used twice", item.id));
        }
        seen.push(&item.id);
    }
    out
}

pub fn skeleton(description: &str) -> String {
    let goal = description.trim();
    let goal = goal.split("\n\n").next().unwrap_or("").trim();
    format!(
        "## Goal\n{}\n\n## How\n\n## Checklist\n- [ ] {{s1}} \n- [ ] {{s2}} (verify) \n",
        if goal.is_empty() {
            "What done looks like."
        } else {
            goal
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = "---\ndescription: Plan\n---\n## Goal\nShip it.\nTwo lines.\n\n## How\nCarefully.\n\n## Checklist\n- [x] {s1} Write the parser\n- [~] {s2} Wire it in\n    detail one\n    detail two\n- [ ] {s3} (verify) `cargo test` passes\n- not a step\n\n## Notes\n- [ ] {n1} not in the checklist\n";

    #[test]
    fn a_plan_is_read_by_its_sections() {
        let p = parse(PLAN);
        assert_eq!(p.goal, "Ship it.\nTwo lines.");
        assert_eq!(p.how, "Carefully.");
        assert_eq!(p.other, ["Notes"]);
        let steps: Vec<_> = p
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.state, i.verify, i.text.as_str()))
            .collect();
        assert_eq!(
            steps,
            [
                ("s1", State::Done, false, "Write the parser"),
                ("s2", State::Doing, false, "Wire it in"),
                ("s3", State::Todo, true, "`cargo test` passes"),
            ]
        );
        assert_eq!(p.items[1].detail, "detail one\ndetail two");
        assert_eq!(p.items[2].detail, "");
        assert_eq!(
            p.progress(),
            Progress {
                done: 1,
                doing: 1,
                total: 3,
                verify_done: 0,
                verify_total: 1
            }
        );
    }

    #[test]
    fn headings_and_steps_inside_code_are_not_structure() {
        let p = parse(
            "## Goal\ng\n```\n## How\n- [ ] x\n```\n## Checklist\n```\n- [ ] {s9} no\n```\n- [ ] {s1} yes\n",
        );
        assert_eq!(p.how, "");
        assert_eq!(p.goal, "g\n```\n## How\n- [ ] x\n```");
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.items[0].id, "s1");
    }

    #[test]
    fn forgiving_about_what_a_person_writes() {
        let p = parse(
            "## goal\nx\n## CHECKLIST\n* [X] {a-1} Star and capital\n- [ ]\n- [ ]no space\n-  [ ] two spaces\n- [ ] {bad id} text\n- [ ] (VERIFY) shouted\n",
        );
        let t: Vec<_> = p
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.text.as_str(), i.verify))
            .collect();
        assert_eq!(p.goal, "x");
        assert_eq!(
            t,
            [
                ("a-1", "Star and capital", false),
                ("", "", false),
                ("", "{bad id} text", false),
                ("", "shouted", true),
            ]
        );
        assert_eq!(p.items[0].state, State::Done);
    }

    #[test]
    fn ticking_a_step_changes_one_character() {
        let done = mark(PLAN, "s3", State::Done).expect("marks");
        assert_eq!(done, PLAN.replace("- [ ] {s3}", "- [x] {s3}"));
        let back = mark(&done, "s3", State::Todo).expect("marks");
        assert_eq!(back, PLAN);
        let doing = mark(PLAN, "s1", State::Doing).expect("marks");
        assert_eq!(doing, PLAN.replace("- [x] {s1}", "- [~] {s1}"));
        let bare = "## Checklist\n- [ ] {s1} only";
        assert_eq!(
            mark(bare, "s1", State::Done).unwrap(),
            "## Checklist\n- [x] {s1} only"
        );
    }

    #[test]
    fn an_unknown_step_says_which_exist() {
        let e = mark(PLAN, "zz", State::Done).unwrap_err();
        assert_eq!(e.to_string(), "no step 'zz' (the steps are s1, s2, s3)");
        let e = mark("## Goal\nx\n", "s1", State::Done).unwrap_err();
        assert_eq!(e.to_string(), "no step 's1': the plan has no steps");
    }

    #[test]
    fn numbering_gives_the_next_free_id_and_changes_nothing_else() {
        let text = "## Checklist\n- [ ] first\n- [x] {s4} kept\n- [~] (verify) third\n  detail\n- [ ] {s4} clash\n";
        let n = normalize(text);
        assert_eq!(
            n,
            "## Checklist\n- [ ] {s5} first\n- [x] {s4} kept\n- [~] {s6} (verify) third\n  detail\n- [ ] {s7} clash\n"
        );
        assert_eq!(normalize(&n), n);
        assert_eq!(normalize(PLAN), PLAN);
    }

    #[test]
    fn an_empty_step_is_numbered_too() {
        assert_eq!(
            normalize("## Checklist\n- [ ]\n- [ ] \n"),
            "## Checklist\n- [ ] {s1}\n- [ ] {s2}\n"
        );
    }

    #[test]
    fn problems_name_what_is_missing() {
        assert!(problems(PLAN).is_empty());
        assert_eq!(
            problems("just prose"),
            ["there is no `## Goal`", "the `## Checklist` has no steps"]
        );
        assert_eq!(
            problems("## Goal\ng\n## Checklist\n- [ ] {s1} a\n- [ ] {s1} b\n"),
            ["step id 's1' is used twice"]
        );
    }

    #[test]
    fn the_skeleton_is_a_plan_with_numbered_steps() {
        let s = skeleton("Make it fast.\n\nMore about it.");
        let p = parse(&s);
        assert_eq!(p.goal, "Make it fast.");
        assert_eq!(p.items.len(), 2);
        assert!(p.items[1].verify);
        let late = parse(
            "## Checklist\n- [ ] run the tests (verify)\n- [ ] look at the diff (Verify: git diff)\n- [ ] verify the thing (not a tag)\n- [ ] run it (verify) then (clean up)\n",
        );
        assert_eq!(
            late.items
                .iter()
                .map(|i| (i.text.as_str(), i.verify))
                .collect::<Vec<_>>(),
            [
                ("run the tests", true),
                ("look at the diff", true),
                ("verify the thing (not a tag)", false),
                ("run it (verify) then (clean up)", false)
            ]
        );
        assert_eq!(parse(&skeleton("")).goal, "What done looks like.");
    }

    #[test]
    fn front_matter_is_not_the_plan() {
        // a `## Goal` inside the front matter's YAML would be a comment, not a heading
        let p = parse("---\ndescription: x\n---\n## Goal\ng\n");
        assert_eq!(p.goal, "g");
        assert_eq!(parse("---\nunclosed\n## Goal\ng\n").goal, "g");
    }

    #[test]
    fn big_steps_are_the_headings_of_the_checklist_and_have_their_own_progress() {
        let p = parse(
            "## Checklist\n- [x] {s1} before any heading\n### develop\n- [x] {s2} a\n- [~] {s3} b\n### test\n- [ ] {s4} c\n",
        );
        assert_eq!(p.phases, ["develop", "test"]);
        let phases: Vec<Option<usize>> = p.items.iter().map(|i| i.phase).collect();
        assert_eq!(phases, [None, Some(0), Some(0), Some(1)]);
        assert_eq!((p.phase_progress(0), p.phase_progress(1)), ((1, 2), (0, 1)));
        assert!(parse("## Checklist\n- [ ] a\n").phases.is_empty());
        let marked = mark(
            "## Checklist\n### develop\n- [ ] {s1} a\n",
            "s1",
            State::Done,
        )
        .unwrap();
        assert_eq!(marked, "## Checklist\n### develop\n- [x] {s1} a\n");
    }
}
