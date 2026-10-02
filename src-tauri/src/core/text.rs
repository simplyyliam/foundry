//! Small text helpers shared by features.

/// Runs of whitespace (including newlines) become one space; trimmed. For comparing a quoted
/// excerpt against its source regardless of line wrapping.
pub fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_whitespace() {
        assert_eq!(normalize_ws("  a \r\n\tb  c "), "a b c");
        assert_eq!(normalize_ws(""), "");
    }
}
