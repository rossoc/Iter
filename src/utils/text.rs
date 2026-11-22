//! Small text helpers several modules need: percent-encoding, a YAML scalar as text, a timestamp.

use serde_yaml::Value;

/// `text` with every byte that is not an ASCII letter, a digit or one of `keep` as `%XX`.
pub fn percent_encode(text: &str, keep: &[u8]) -> String {
    use std::fmt::Write as _;
    text.bytes()
        .fold(String::with_capacity(text.len()), |mut out, b| {
            match b.is_ascii_alphanumeric() || keep.contains(&b) {
                true => out.push(b as char),
                false => {
                    let _ = write!(out, "%{b:02X}");
                }
            }
            out
        })
}

/// A YAML string, number or boolean as text; anything else is not a scalar.
pub fn yaml_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The local time now, formatted with the `chrono` format `format`.
pub fn now_as(format: &str) -> String {
    chrono::Local::now().format(format).to_string()
}
