//! Parse a user-supplied repo argument into a canonical **identity** and a
//! **clone URL**.
//!
//! The identity (`<host>/<owner>/<repo>`) is what the manifest, lock, and Cache
//! path key on (the ghq/go convention); the clone URL is what `git` fetches
//! from. For a plain `owner/repo` shorthand the URL is derived as GitHub HTTPS;
//! for an explicit URL the identity is parsed back out of it.

use crate::error::{Error, Result};

/// A parsed vendored source: its canonical identity and clone URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Canonical identity `<host>/<owner>/<repo>` — the manifest/lock/Cache key.
    pub repo: String,
    /// The URL `git clone` fetches from.
    pub url: String,
}

/// Parse a repo argument. Accepts `owner/repo` shorthand, a bare
/// `host/owner/repo` identity, or an `https`/`ssh`/`git@` URL.
pub fn parse_source(arg: &str) -> Result<Source> {
    if let Some(rest) = arg.strip_prefix("git@") {
        // git@host:owner/repo(.git)
        let (host, path) = rest.split_once(':').ok_or_else(|| bad(arg))?;
        let identity = format!("{host}/{}", trim_git(path));
        require_triple(&identity, arg)?;
        return Ok(Source {
            repo: identity,
            url: arg.to_string(),
        });
    }

    if let Some((_scheme, rest)) = arg.split_once("://") {
        // scheme://[user@]host/owner/repo(.git)
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let identity = trim_git(rest.trim_end_matches('/'));
        require_triple(identity, arg)?;
        return Ok(Source {
            repo: identity.to_string(),
            url: arg.to_string(),
        });
    }

    // No scheme: a bare identity or shorthand.
    let path = trim_git(arg.trim_matches('/'));
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.len() {
        2 => Ok(Source {
            repo: format!("github.com/{path}"),
            url: format!("https://github.com/{path}.git"),
        }),
        n if n >= 3 => Ok(Source {
            repo: path.to_string(),
            url: format!("https://{path}.git"),
        }),
        _ => Err(bad(arg)),
    }
}

/// Strip a trailing `.git` suffix.
fn trim_git(s: &str) -> &str {
    s.strip_suffix(".git").unwrap_or(s)
}

/// Require an identity of exactly `host/owner/repo` (three segments).
fn require_triple(identity: &str, arg: &str) -> Result<()> {
    if identity.split('/').filter(|s| !s.is_empty()).count() >= 3 {
        Ok(())
    } else {
        Err(bad(arg))
    }
}

fn bad(arg: &str) -> Error {
    Error::Invalid(format!(
        "unrecognized repo '{arg}' (want owner/repo, host/owner/repo, or a git URL)"
    ))
}

#[cfg(test)]
mod tests {
    use super::{parse_source, Source};

    fn src(repo: &str, url: &str) -> Source {
        Source {
            repo: repo.into(),
            url: url.into(),
        }
    }

    #[test]
    fn owner_repo_shorthand_becomes_github_https() {
        assert_eq!(
            parse_source("mattpocock/skills").unwrap(),
            src(
                "github.com/mattpocock/skills",
                "https://github.com/mattpocock/skills.git"
            )
        );
    }

    #[test]
    fn bare_host_owner_repo_derives_https() {
        assert_eq!(
            parse_source("gitlab.com/group/proj").unwrap(),
            src("gitlab.com/group/proj", "https://gitlab.com/group/proj.git")
        );
    }

    #[test]
    fn https_url_keeps_url_and_parses_identity() {
        assert_eq!(
            parse_source("https://github.com/mattpocock/skills").unwrap(),
            src(
                "github.com/mattpocock/skills",
                "https://github.com/mattpocock/skills"
            )
        );
        assert_eq!(
            parse_source("https://github.com/mattpocock/skills.git")
                .unwrap()
                .repo,
            "github.com/mattpocock/skills"
        );
    }

    #[test]
    fn ssh_shorthand_parses_identity() {
        assert_eq!(
            parse_source("git@github.com:mattpocock/skills.git").unwrap(),
            src(
                "github.com/mattpocock/skills",
                "git@github.com:mattpocock/skills.git"
            )
        );
    }

    #[test]
    fn rejects_a_lone_segment() {
        assert!(parse_source("justone").is_err());
    }
}
