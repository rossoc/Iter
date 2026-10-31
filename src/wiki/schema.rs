//! The instructions an agent gets (`iter wiki schema`): `schema.md` with the ontology-derived parts filled in, so they cannot drift from `check`.

use super::ontology::{NS, Ontology, ns};
use oxigraph::model::TermRef;
use oxigraph::model::vocab::rdfs;

const TEXT: &str = include_str!("schema.md");

fn comment(onto: &Ontology, subject: &oxigraph::model::NamedNode) -> Option<String> {
    onto.graph
        .objects_for_subject_predicate(subject, rdfs::COMMENT)
        .find_map(|t| match t {
            TermRef::Literal(l) => Some(l.value().to_string()),
            _ => None,
        })
}

pub fn render(onto: &Ontology) -> String {
    let mut places = String::from("| Class | Folder | In |\n|---|---|---|\n");
    for p in &onto.places {
        let class = p
            .class
            .as_str()
            .strip_prefix(NS)
            .unwrap_or(p.class.as_str());
        let folder = if p.folder == "." {
            "(the root, files named like a date)".to_string()
        } else {
            format!("`{}`", p.folder)
        };
        let what = comment(onto, &p.class).unwrap_or_default();
        places.push_str(&format!(
            "| {class} | {folder} | {} level. {what} |\n",
            p.kind.key()
        ));
    }
    let mut relations = String::new();
    for (key, prop) in &onto.authorable {
        let what = comment(onto, prop).unwrap_or_default();
        relations.push_str(&format!("- `{key}`: {what}\n"));
    }
    let confidence = match onto.allowed_values(&ns("confidence")) {
        Some(values) => format!("one of {}", values.join(", ")),
        None => "free text".to_string(),
    };
    TEXT.replace("{{places}}", places.trim_end())
        .replace("{{confidence}}", &confidence)
        .replace("{{relations}}", relations.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_come_from_the_ontology() {
        let text = render(&Ontology::load(None).expect("loads"));
        assert!(text.contains("| Idea | `wiki/ideas` | organization level."));
        assert!(text.contains("| Plan | `plan.md` | task level."));
        assert!(text.contains("- `nb`: Reference that matters"));
        assert!(text.contains("- `confidence` is one of high, medium, low."));
        assert!(!text.contains("{{"));
    }

    #[test]
    fn an_extension_shows_up() {
        let ext = "@prefix iter: <urn:iter:ns:> . @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
                   iter:Decision rdfs:subClassOf iter:Page ; rdfs:comment \"A choice and why.\" ; iter:place \"project:wiki/decisions\" .";
        let text = render(&Ontology::load(Some(ext)).expect("loads"));
        assert!(
            text.contains("| Decision | `wiki/decisions` | project level. A choice and why. |")
        );
    }
}
