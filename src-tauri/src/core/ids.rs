//! Id generation. UUID v4, rendered as lowercase hyphenated strings.

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_uuid_strings() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert!(uuid::Uuid::parse_str(&a).is_ok());
    }
}
