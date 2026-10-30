//! A page's front matter and body links.
//!
//! The front matter is strict YAML between a first line `---` and the next
//! one. A page may have none (then it has no description, which `check` reports);
//! a note is never parsed at all. Besides the four data keys, any key the
//! ontology marks `iter:authorable` is a relation, and its value is a target
//! or a list of them. Any other key is kept in `unknown` so `check` can say so.

use crate::md_edit::split_at_divider;
use serde_yaml::Value;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Front {
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub confidence: Option<String>,
    pub updated: Option<String>,
    /// `(key, targets)` in the order written, for the keys the ontology
    /// allows as relations.
    pub relations: Vec<(String, Vec<String>)>,
    /// Keys that are neither data nor a known relation.
    pub unknown: Vec<String>,
}

/// A page split into the YAML text between the dividers (if there is a
/// front matter) and the markdown below it.
pub struct Split<'a> {
    pub yaml: Option<&'a str>,
    pub body: &'a str,
}

/// Splits `content`. The front matter must open on the first line: a `---`
/// further down is a rule in the markdown, not a divider.
pub fn split(content: &str) -> Split<'_> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let Some(rest) = content.strip_prefix("---\n") else {
        return Split {
            yaml: None,
            body: content,
        };
    };
    match split_at_divider(rest) {
        Some((yaml, body)) => Split {
            yaml: Some(yaml),
            body,
        },
        None => Split {
            yaml: None,
            body: content,
        },
    }
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// One value or a list of them, as strings; `None` if it is neither.
fn strings(v: &Value) -> Option<Vec<String>> {
    match v {
        Value::Sequence(items) => items.iter().map(scalar).collect(),
        Value::Null => Some(Vec::new()),
        other => scalar(other).map(|s| vec![s]),
    }
}

/// Reads `yaml`. `relations` are the keys allowed as relations (the
/// ontology's authorable properties). Fails, saying why, if the text is not a
/// mapping or a value has the wrong shape.
pub fn parse(yaml: &str, relations: &[&str]) -> Result<Front, String> {
    let value: Value = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
    let map = match value {
        Value::Mapping(map) => map,
        Value::Null => return Ok(Front::default()),
        _ => return Err("the front matter is not a mapping".into()),
    };
    let mut front = Front::default();
    for (key, value) in &map {
        let Some(key) = key.as_str() else {
            return Err("a front matter key is not text".into());
        };
        let wrong = || format!("`{key}` has the wrong shape");
        // `description:` with nothing after it is a key left empty, not a mistake
        if value.is_null() && matches!(key, "description" | "confidence" | "updated") {
            continue;
        }
        match key {
            "description" => front.description = Some(scalar(value).ok_or_else(wrong)?),
            "confidence" => front.confidence = Some(scalar(value).ok_or_else(wrong)?),
            "updated" => front.updated = Some(scalar(value).ok_or_else(wrong)?),
            "tags" => front.tags = strings(value).ok_or_else(wrong)?,
            k if relations.contains(&k) => front
                .relations
                .push((k.to_string(), strings(value).ok_or_else(wrong)?)),
            k => front.unknown.push(k.to_string()),
        }
    }
    Ok(front)
}

/// The targets of the `[[wikilinks]]` in `body`, in order, outside fenced
/// code. `[[target|label]]` and `[[target#heading]]` link to `target`; a
/// trailing `.md` is dropped.
pub fn wikilinks(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut rest = line;
        while let Some(open) = rest.find("[[") {
            let after = &rest[open + 2..];
            let Some(close) = after.find("]]") else { break };
            let target = after[..close]
                .split(['|', '#'])
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(".md");
            if !target.is_empty() {
                out.push(target.to_string());
            }
            rest = &after[close + 2..];
        }
    }
    out
}

/// The first `# Heading` of `body`: what a page is called.
pub fn title(body: &str) -> Option<String> {
    body.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: &[&str] = &["source", "include", "nb"];

    #[test]
    fn front_matter_opens_on_the_first_line() {
        let s = split("---\ndescription: x\n---\nbody\n");
        assert_eq!((s.yaml, s.body), (Some("description: x\n"), "body\n"));
        let s = split("intro\n\n---\nnot: front\n---\n");
        assert_eq!(s.yaml, None);
        assert!(s.body.starts_with("intro"));
        assert_eq!(split("---\nnever closed\n").yaml, None);
    }

    #[test]
    fn data_and_relations_are_read() {
        let f = parse(
            "description: Retries need a budget\ntags: [net, retry]\nconfidence: high\nupdated: 2026-10-27\nsource: [2026-10-27]\nnb: concepts/backoff\n",
            KEYS,
        )
        .expect("parses");
        assert_eq!(f.description.as_deref(), Some("Retries need a budget"));
        assert_eq!(f.tags, ["net", "retry"]);
        assert_eq!(f.updated.as_deref(), Some("2026-10-27"));
        assert_eq!(
            f.relations,
            [
                ("source".to_string(), vec!["2026-10-27".to_string()]),
                ("nb".to_string(), vec!["concepts/backoff".to_string()])
            ]
        );
        assert!(f.unknown.is_empty());
    }

    #[test]
    fn unknown_keys_and_bad_shapes_are_reported() {
        let f = parse("description: a\nmood: happy\n", KEYS).expect("parses");
        assert_eq!(f.unknown, ["mood"]);
        assert!(parse("description: [a, b]\n", KEYS).is_err());
        assert!(parse("- just\n- a list\n", KEYS).is_err());
        assert_eq!(parse("", KEYS).expect("empty"), Front::default());
    }

    #[test]
    fn links_skip_code_and_take_the_target() {
        let body = "see [[concepts/a]] and [[b.md|the b]], [[c#part]]\n```\n[[not-a-link]]\n```\n[[ ]] [[d]]";
        assert_eq!(wikilinks(body), ["concepts/a", "b", "c", "d"]);
    }

    #[test]
    fn a_title_is_the_first_heading() {
        assert_eq!(title("x\n# Retries\n# Other").as_deref(), Some("Retries"));
        assert_eq!(title("## sub only"), None);
    }

    #[test]
    fn a_key_left_empty_is_absent_not_a_mistake() {
        let f = parse(
            "description:\ntags: [a]\nsource: n1\nupdated:\n",
            &["source"],
        )
        .unwrap();
        assert_eq!((f.description, f.updated), (None, None));
        assert_eq!(
            f.relations,
            [("source".to_string(), vec!["n1".to_string()])]
        );
    }
}
