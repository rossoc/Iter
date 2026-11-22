//! The count badge of a heading; a screen reader hears "(3 sessions)".

use topcoat::{
    Result,
    view::{View, component, view},
};

/// Screen-reader text: "1 task", "3 tasks".
fn counted(n: usize, noun: &str) -> String {
    let noun = if n == 1 {
        noun.strip_suffix('s').unwrap_or(noun)
    } else {
        noun
    };
    format!("{n} {noun}")
}

/// `noun` is the plural; without one the badge is the bare number.
#[component]
pub async fn count_badge(n: usize, #[default] noun: &str) -> Result<impl View> {
    Ok(view! {
        if noun.is_empty() {
            <span class="count">(n)</span>
        } else {
            <span class="count"><span aria-hidden="true">(n)</span><span class="sr">" (" (counted(n, noun)) ")"</span></span>
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_is_singular() {
        assert_eq!(counted(1, "tasks"), "1 task");
        assert_eq!(counted(0, "tasks"), "0 tasks");
        assert_eq!(counted(3, "sessions"), "3 sessions");
    }
}
