
use crate::error::IterError;

/// `field` is the `name` of the control the error is about, if any; controls and the error notice use it.
pub struct FormError {
    pub message: String,
    pub field: Option<&'static str>,
}

impl FormError {
    pub fn new(error: &IterError, field: Option<&'static str>) -> FormError {
        FormError {
            message: sentence(&error.to_string()),
            field,
        }
    }

    pub fn is_about(&self, name: &str) -> bool {
        self.field == Some(name)
    }
}

/// `message` as a capitalised sentence with a period (the CLI prints fragments).
pub fn sentence(message: &str) -> String {
    let message = message.trim().replace(" github ", " GitHub ");
    let mut chars = message.chars();
    let mut out: String = chars
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    out.push_str(chars.as_str());
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

pub fn is_about(error: Option<&FormError>, name: &str) -> bool {
    error.is_some_and(|e| e.is_about(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_says_its_message_and_field() {
        let e = FormError::new(&IterError::EmptyName("task"), Some("name"));
        assert!(e.is_about("name") && !e.message.is_empty());
    }

    #[test]
    fn a_message_is_a_capitalised_sentence() {
        assert_eq!(
            sentence("a task with this name already exists"),
            "A task with this name already exists."
        );
        assert_eq!(
            sentence("Start time is not valid."),
            "Start time is not valid."
        );
        assert_eq!(
            sentence("project 'x' has github disabled"),
            "Project 'x' has GitHub disabled."
        );
        assert_eq!(sentence(""), ".");
    }

    #[test]
    fn an_error_is_about_only_its_own_field() {
        let e = FormError {
            message: "m".into(),
            field: Some("name"),
        };
        assert!(is_about(Some(&e), "name"));
        assert!(!is_about(Some(&e), "projects"));
        assert!(!is_about(None, "name"));
        let general = FormError {
            message: "m".into(),
            field: None,
        };
        assert!(!is_about(Some(&general), "name"));
    }
}
