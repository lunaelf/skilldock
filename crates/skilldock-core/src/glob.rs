/// Whether a declared path is a glob pattern rather than an exact subpath.
///
/// Globs are legal only in the declared manifest (`skilldock.toml`); the lock
/// always holds exact, expanded entries. Detection is purely syntactic — the
/// presence of a shell glob metacharacter (`*`, `?`, or a `[...]` class).
pub fn is_glob(path: &str) -> bool {
    path.contains(['*', '?', '['])
}

#[cfg(test)]
mod tests {
    use super::is_glob;

    #[test]
    fn plain_paths_are_not_globs() {
        assert!(!is_glob("skills/engineering/grilling"));
        assert!(!is_glob("git-commit"));
    }

    #[test]
    fn metacharacters_mark_globs() {
        assert!(is_glob("skills/engineering/*"));
        assert!(is_glob("skills/**/domain"));
        assert!(is_glob("skills/eng?"));
        assert!(is_glob("skills/[abc]"));
    }
}
