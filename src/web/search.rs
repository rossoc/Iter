//! The palette's search (`ui/search.rs`): a query parsed and ranked against
//! the pages of the app. Pure, no markup, no database.
//!
//! A query is `[kind ]words`, optionally with scopes before it, separated by
//! `/`:
//!
//! | query | meaning |
//! |---|---|
//! | `proj ` | every project (`task `, `org `, `board `, `note ` likewise) |
//! | `task fix` | tasks whose name matches `fix` |
//! | `org acme/proj web` | projects named like `web` that sit under an organization named like `acme` |
//! | `fix login` | every page: all the words in the title first, then the ones that need the description |
//! | `is:wip` | tasks in one status (`queue`, `wip`, `done`) |
//!
//! A kind word is any prefix of `board`, `organization`, `project`, `task`
//! or `note`, followed by a blank. A title matches by substring (best at the
//! start, then at a word start) or, failing that, as a subsequence (`wd`
//! finds `write docs`).

/// What a page is, in the order the results list them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Board,
    Org,
    Project,
    Task,
    Note,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Board, Kind::Org, Kind::Project, Kind::Task, Kind::Note];

    /// The word that completes to this kind, and its label.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Board => "board",
            Kind::Org => "org",
            Kind::Project => "project",
            Kind::Task => "task",
            Kind::Note => "note",
        }
    }

    /// What a kind word may be a prefix of; `org` is short for the
    /// organization.
    fn names(self) -> &'static [&'static str] {
        match self {
            Kind::Board => &["board"],
            Kind::Org => &["organization", "org"],
            Kind::Project => &["project"],
            Kind::Task => &["task"],
            Kind::Note => &["note", "notes"],
        }
    }

    fn of_word(word: &str) -> Option<Kind> {
        let word = word.to_lowercase();
        Kind::ALL
            .into_iter()
            .find(|k| k.names().iter().any(|n| n.starts_with(&word)))
    }
}

/// A page the search can lead to.
#[derive(Debug, Clone)]
pub struct Entry {
    pub kind: Kind,
    pub title: String,
    /// The pages above it, outermost first: an organization, a project.
    pub ancestors: Vec<(Kind, String)>,
    pub url: String,
    /// Markdown, searched after the title.
    pub description: String,
    /// A task's status (`queue`, `wip`, `done`), else empty.
    pub status: String,
}

/// A parsed query.
#[derive(Debug, PartialEq, Default)]
pub struct Query {
    /// The pages the target must sit under: a kind (or any) and a name.
    scopes: Vec<(Option<Kind>, String)>,
    kind: Option<Kind>,
    /// Lowercased words to find.
    words: Vec<String>,
    status: Option<String>,
}

/// `kind rest` -> `(Some(kind), rest)` when the first word names a kind and a
/// blank follows it; otherwise `(None, segment)`.
fn split_kind(segment: &str) -> (Option<Kind>, &str) {
    let segment = segment.trim_start();
    match segment.split_once(char::is_whitespace) {
        Some((word, rest)) => match Kind::of_word(word) {
            Some(kind) => (Some(kind), rest),
            None => (None, segment),
        },
        None => (None, segment),
    }
}

impl Query {
    pub fn parse(q: &str) -> Query {
        let mut segments: Vec<&str> = q.split('/').collect();
        let last = segments.pop().unwrap_or_default();
        let scopes = segments
            .into_iter()
            .map(|s| {
                let (kind, rest) = split_kind(s);
                (kind, rest.trim().to_lowercase())
            })
            .collect();
        let (kind, rest) = split_kind(last);
        let mut query = Query {
            scopes,
            kind,
            ..Query::default()
        };
        for word in rest.split_whitespace() {
            match word.strip_prefix("is:") {
                Some(status) if ["queue", "wip", "done"].contains(&status) => {
                    query.status = Some(status.to_string());
                    query.kind = query.kind.or(Some(Kind::Task));
                }
                _ => query.words.push(word.to_lowercase()),
            }
        }
        query
    }
}

/// How well `word` matches `text` (both lowercased), and the char indexes it
/// covers: a prefix beats a word start beats a substring beats a subsequence.
fn fuzzy(text: &str, word: &str) -> Option<(i32, Vec<usize>)> {
    let t: Vec<char> = text.chars().collect();
    let w: Vec<char> = word.chars().collect();
    if w.is_empty() {
        return Some((0, Vec::new()));
    }
    if let Some(at) = t.windows(w.len()).position(|win| win == w.as_slice()) {
        let starts_word = at == 0 || !t[at - 1].is_alphanumeric();
        let score = match (at, starts_word) {
            (0, _) => 3000,
            (_, true) => 2000,
            _ => 1000,
        } - at as i32;
        return Some((score, (at..at + w.len()).collect()));
    }
    let (mut at, mut found, mut score, mut prev) = (0, Vec::new(), 100, None::<usize>);
    for c in &w {
        let i = at + t[at..].iter().position(|x| x == c)?;
        if prev.is_some_and(|p| p + 1 == i) {
            score += 10;
        }
        found.push(i);
        prev = Some(i);
        at = i + 1;
    }
    Some((score, found))
}

/// A matching page: lower `tier` is better (0: every word is in the title).
#[derive(Debug)]
pub struct Hit<'a> {
    pub entry: &'a Entry,
    tier: u8,
    score: i32,
    /// Char indexes of the title the query matched, for highlighting.
    pub marks: Vec<usize>,
}

fn hit<'a>(entry: &'a Entry, q: &Query) -> Option<Hit<'a>> {
    if q.kind.is_some_and(|k| k != entry.kind)
        || q.status.as_deref().is_some_and(|s| s != entry.status)
    {
        return None;
    }
    for (kind, name) in &q.scopes {
        let under = entry.ancestors.iter().any(|(k, title)| {
            kind.is_none_or(|want| want == *k) && fuzzy(&title.to_lowercase(), name).is_some()
        });
        if !under {
            return None;
        }
    }
    let title = entry.title.to_lowercase();
    let description = entry.description.to_lowercase();
    let (mut tier, mut score, mut marks) = (0, 0, Vec::new());
    for word in &q.words {
        if let Some((s, found)) = fuzzy(&title, word) {
            score += s;
            marks.extend(found);
        } else if description.contains(word.as_str()) {
            tier = 1;
        } else {
            return None;
        }
    }
    marks.sort_unstable();
    marks.dedup();
    Some(Hit {
        entry,
        tier,
        score,
        marks,
    })
}

/// The pages `query` finds, best first, at most `limit`.
pub fn search<'a>(entries: &'a [Entry], query: &str, limit: usize) -> Vec<Hit<'a>> {
    let q = Query::parse(query);
    let mut hits: Vec<Hit> = entries.iter().filter_map(|e| hit(e, &q)).collect();
    hits.sort_by(|a, b| {
        (a.tier, -a.score, a.entry.kind, a.entry.title.to_lowercase()).cmp(&(
            b.tier,
            -b.score,
            b.entry.kind,
            b.entry.title.to_lowercase(),
        ))
    });
    hits.truncate(limit);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: Kind, title: &str, ancestors: &[(Kind, &str)], description: &str) -> Entry {
        Entry {
            kind,
            title: title.to_string(),
            ancestors: ancestors.iter().map(|(k, t)| (*k, t.to_string())).collect(),
            url: format!("/{title}"),
            description: description.to_string(),
            status: String::new(),
        }
    }

    fn titles<'a>(hits: &[Hit<'a>]) -> Vec<&'a str> {
        hits.iter().map(|h| h.entry.title.as_str()).collect()
    }

    fn sample() -> Vec<Entry> {
        let mut wip = entry(Kind::Task, "fix login", &[(Kind::Org, "acme"), (Kind::Project, "web")], "");
        wip.status = "wip".into();
        vec![
            entry(Kind::Org, "acme", &[], "the customer"),
            entry(Kind::Project, "web", &[(Kind::Org, "acme")], "login and billing"),
            entry(Kind::Project, "api", &[(Kind::Org, "beta")], ""),
            wip,
            entry(Kind::Task, "write docs", &[(Kind::Project, "api")], "explain the login flow"),
            entry(Kind::Board, "work", &[], ""),
        ]
    }

    #[test]
    fn a_kind_word_restricts_to_that_kind() {
        let all = sample();
        assert_eq!(titles(&search(&all, "proj ", 10)), ["api", "web"]);
        assert_eq!(titles(&search(&all, "t ", 10)), ["fix login", "write docs"]);
        assert_eq!(titles(&search(&all, "task fix", 10)), ["fix login"]);
    }

    #[test]
    fn without_a_blank_the_word_is_text() {
        let all = sample();
        assert!(search(&all, "proj", 10).is_empty());
    }

    #[test]
    fn the_title_beats_the_description() {
        let all = sample();
        let hits = search(&all, "login", 10);
        // the task titled "fix login" first, then the ones that only mention it
        assert_eq!(titles(&hits), ["fix login", "web", "write docs"]);
    }

    #[test]
    fn a_scope_names_a_page_above() {
        let all = sample();
        assert_eq!(titles(&search(&all, "org acme/proj ", 10)), ["web"]);
        assert_eq!(titles(&search(&all, "acme/task ", 10)), ["fix login"]);
        assert!(search(&all, "org nope/proj ", 10).is_empty());
    }

    #[test]
    fn a_status_finds_tasks() {
        let all = sample();
        assert_eq!(titles(&search(&all, "is:wip", 10)), ["fix login"]);
    }

    #[test]
    fn a_subsequence_matches_after_substrings() {
        let all = vec![
            entry(Kind::Task, "write docs", &[], ""),
            entry(Kind::Task, "wd", &[], ""),
        ];
        assert_eq!(titles(&search(&all, "wd", 10)), ["wd", "write docs"]);
        assert!(search(&all, "xyz", 10).is_empty());
    }

    #[test]
    fn an_empty_query_lists_everything_by_kind_then_title() {
        let all = sample();
        let hits = search(&all, "", 3);
        assert_eq!(titles(&hits), ["work", "acme", "api"]);
    }

    #[test]
    fn marks_cover_the_matched_title_chars() {
        let all = vec![entry(Kind::Task, "fix login", &[], "")];
        assert_eq!(search(&all, "log", 10)[0].marks, [4, 5, 6]);
    }

    #[test]
    fn the_script_completes_the_same_kinds() {
        let words: Vec<String> = Kind::ALL.iter().map(|k| format!("\"{}\"", k.word())).collect();
        let list = format!("var KINDS = [{}];", words.join(","));
        assert!(include_str!("ui/search.js").contains(&list));
    }
}
