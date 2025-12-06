//! Text processing utilities.

/// Minimum non-whitespace characters for meaningful content.
pub const MIN_CONTENT_LENGTH: usize = 50;

/// Check if content has meaningful text (not just whitespace/punctuation).
pub fn has_meaningful_content(content: &str) -> bool {
    content.chars().filter(|c| !c.is_whitespace()).count() >= MIN_CONTENT_LENGTH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_meaningful_content() {
        assert!(!has_meaningful_content(""));
        assert!(!has_meaningful_content("   \n\n   "));
        assert!(!has_meaningful_content("short"));
        assert!(!has_meaningful_content(&" ".repeat(1000)));
        assert!(!has_meaningful_content("| | | |"));
        assert!(has_meaningful_content(&"a".repeat(50)));
        assert!(has_meaningful_content(
            "This is a meaningful piece of content with enough characters."
        ));
    }
}
