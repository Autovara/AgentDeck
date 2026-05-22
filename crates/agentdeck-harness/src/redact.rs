use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;

/// Redaction rules applied before a fixture is committed to the repo.
///
/// See `planning/agent-harness-notes.md` §4 for the full list of rules.
/// The redactor is deliberately conservative: it may over-redact, but it
/// must never silently leave a captured secret or personal path
/// untouched.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// Absolute path of the recorder's home directory, replaced with
    /// `<HOME>` everywhere it appears.
    pub home: Option<PathBuf>,
    /// Absolute path of the active repo, replaced with `<REPO>`.
    pub repo: Option<PathBuf>,
    /// Username to redact in any path or text. Replaced with
    /// `<REDACTED:user>`.
    pub username: Option<String>,
}

impl Redactor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    pub fn with_repo(mut self, repo: impl Into<PathBuf>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Apply every redaction rule to a piece of text.
    pub fn redact(&self, input: &str) -> String {
        redact_text(input, self)
    }
}

/// Apply redaction rules to a string.
///
/// The function is pure: it does not touch the filesystem or
/// environment, so callers can reason about its behavior in tests.
///
/// Ordering matters: longer-prefix paths are redacted first (`repo`
/// before `home`), emails are redacted before the username so addresses
/// containing the username survive intact, and the username pass runs
/// last so it never corrupts patterns redacted earlier.
pub fn redact_text(input: &str, r: &Redactor) -> String {
    let mut out = input.to_string();

    if let Some(repo) = r.repo.as_ref().and_then(|p| p.to_str()) {
        out = out.replace(repo, "<REPO>");
    }
    if let Some(home) = r.home.as_ref().and_then(|p| p.to_str()) {
        out = out.replace(home, "<HOME>");
    }

    out = redact_emails(&out);
    out = redact_bearer(&out);
    out = redact_api_keys(&out);

    if let Some(user) = r.username.as_deref() {
        if !user.is_empty() {
            out = out.replace(user, "<REDACTED:user>");
        }
    }

    out
}

fn email_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b")
            .expect("email regex compiles")
    })
}

fn bearer_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"Bearer\s+\S+").expect("bearer regex compiles"))
}

fn api_key_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Conservative matcher for common provider key prefixes.
        // Provider-specific patterns can be added as adapters land.
        Regex::new(
            r"\b(?:sk-[A-Za-z0-9_\-]{17,}|sk_[A-Za-z0-9_\-]{17,}|xoxb-[A-Za-z0-9_\-]{17,})\b",
        )
        .expect("api key regex compiles")
    })
}

fn redact_emails(s: &str) -> String {
    email_re().replace_all(s, "<REDACTED:email>").into_owned()
}

fn redact_bearer(s: &str) -> String {
    bearer_re()
        .replace_all(s, "Bearer <REDACTED:secret>")
        .into_owned()
}

fn redact_api_keys(s: &str) -> String {
    api_key_re()
        .replace_all(s, "<REDACTED:secret>")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_repo_then_home() {
        let r = Redactor::new()
            .with_home("/home/dev")
            .with_repo("/home/dev/projects/billing-api");
        let input = "Project at /home/dev/projects/billing-api/src and home is /home/dev/.cache";
        let got = r.redact(input);
        assert_eq!(got, "Project at <REPO>/src and home is <HOME>/.cache");
    }

    #[test]
    fn redacts_bearer_tokens() {
        let r = Redactor::new();
        let got = r.redact("Authorization: Bearer abc123 token");
        assert_eq!(got, "Authorization: Bearer <REDACTED:secret> token");
    }

    #[test]
    fn redacts_sk_dash_keys() {
        let r = Redactor::new();
        let got = r.redact("export OPENAI_KEY=sk-1234567890abcdefghij");
        assert_eq!(got, "export OPENAI_KEY=<REDACTED:secret>");
    }

    #[test]
    fn does_not_redact_short_sk_strings() {
        let r = Redactor::new();
        let got = r.redact("write sk-short here");
        assert_eq!(got, "write sk-short here");
    }

    #[test]
    fn redacts_emails() {
        let r = Redactor::new();
        let got = r.redact("contact alice@example.com please");
        assert_eq!(got, "contact <REDACTED:email> please");
    }

    #[test]
    fn redacts_username_last_so_emails_survive() {
        let r = Redactor::new().with_username("alice");
        let got = r.redact("alice ran aider; mail: alice@example.com");
        // The email pattern is redacted first, so the username pass
        // does not corrupt it. Standalone "alice" still becomes the
        // username placeholder.
        assert_eq!(got, "<REDACTED:user> ran aider; mail: <REDACTED:email>");
    }

    #[test]
    fn keeps_innocuous_text_unchanged() {
        let r = Redactor::new();
        let got = r.redact("aider running in billing-api");
        assert_eq!(got, "aider running in billing-api");
    }

    #[test]
    fn empty_username_field_is_noop() {
        let r = Redactor::new().with_username("");
        let got = r.redact("alice ran aider");
        assert_eq!(got, "alice ran aider");
    }
}
