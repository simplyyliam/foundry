//! Matching repo-relative paths against the role globs in `agents/roles.json`. Std only: the
//! `foundry-hook` binary includes this file too (`#[path]`), so it must never use `crate::`.
//!
//! `/`-separated, case-sensitive. `*` and `?` match within one segment, `**` matches any
//! number of segments (including none), and a trailing `/` means everything below. A glob
//! with no `/` also matches a file of that name at any depth (as in `.gitignore`).

pub fn matches(glob: &str, path: &str) -> bool {
    let glob = glob.trim();
    let glob = glob.strip_prefix("./").unwrap_or(glob);
    let mut pattern: Vec<&str> = glob.split('/').filter(|s| !s.is_empty()).collect();
    if pattern.is_empty() {
        return false;
    }
    if glob.ends_with('/') {
        pattern.push("**");
    }
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if !glob.contains('/') {
        if let (Some(name), [only]) = (parts.last(), pattern.as_slice()) {
            if segment(only, name) {
                return true;
            }
        }
    }
    segments(&pattern, &parts)
}

fn segments(pattern: &[&str], parts: &[&str]) -> bool {
    match pattern.split_first() {
        None => parts.is_empty(),
        Some((&"**", rest)) => (0..=parts.len()).any(|skip| segments(rest, &parts[skip..])),
        Some((first, rest)) => parts
            .split_first()
            .is_some_and(|(part, others)| segment(first, part) && segments(rest, others)),
    }
}

/// `*` and `?` wildcards within one segment.
fn segment(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || (p[pi] != '*' && p[pi] == t[ti])) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn globs_match_like_the_role_files_mean_them() {
        for (glob, path, expected) in [
            ("agents/**", "agents/roles/a.md", true),
            ("agents/**", "agents", true),
            ("agents/**", "agentsx/a.md", false),
            ("docs/design/**", "docs/design/tokens/c.json", true),
            ("AGENTS.md", "AGENTS.md", true),
            ("AGENTS.md", "sub/AGENTS.md", true),
            ("AGENTS.md", "agents.md", false),
            (".foundry/plan.json", ".foundry/plan.json", true),
            (".foundry/plan.json", ".foundry/plan.jsonx", false),
            ("src/*.rs", "src/main.rs", true),
            ("src/*.rs", "src/a/main.rs", false),
            ("src/**/*.rs", "src/a/b/main.rs", true),
            ("src/**/*.rs", "src/main.rs", true),
            ("*.lock", "deep/Cargo.lock", true),
            ("src/", "src/a/b.ts", true),
            ("./src/?.ts", "src/a.ts", true),
            ("", "anything", false),
        ] {
            assert_eq!(matches(glob, path), expected, "{glob} vs {path}");
        }
    }
}
