use regex::Regex;

#[derive(Debug, Clone, Copy, Default)]
pub struct SecretRedactor;

impl SecretRedactor {
    pub fn redact(&self, input: &str) -> String {
        let mut redacted = input.to_owned();
        for (pattern, replacement) in [
            (
                r"(?i)authorization:\s*bearer\s+[A-Za-z0-9._\-+/=]+",
                "Authorization: Bearer [REDACTED]",
            ),
            (
                r"(?i)(api[_-]?key|token|secret)\s*[=:]\s*[A-Za-z0-9._\-+/=]{8,}",
                "$1=[REDACTED]",
            ),
            (r"sk-[A-Za-z0-9_-]{8,}", "[REDACTED]"),
            (
                r#"/home/[^\s'"`]+/\.config/pi/[^\s'"`]+"#,
                "[REDACTED_PI_AUTH_PATH]",
            ),
        ] {
            let regex = Regex::new(pattern).expect("redaction pattern compiles");
            redacted = regex.replace_all(&redacted, replacement).into_owned();
        }
        redacted
    }
}
