//! Turning user-typed names into file-name-safe slugs.

/// Lowercase letters and digits; every other run becomes one `-`; no leading or trailing `-`.
/// "My Cool App!" → "my-cool-app". May return an empty string ("!!!" → "").
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            slug.push(ch);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies() {
        assert_eq!(slugify("My Cool App!"), "my-cool-app");
        assert_eq!(slugify("  a__b--c  "), "a-b-c");
        assert_eq!(slugify("Café 2"), "café-2");
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("!!!"), "");
    }
}
