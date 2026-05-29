use autocomplete_core::{CompletionProvider, ProviderError, ProviderOutput};
use autocomplete_protocol::AutocompleteRequest;
use autocomplete_provider_pi::{PiProvider, PiProviderConfig, SecretRedactor};
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn fixture_request() -> AutocompleteRequest {
    serde_json::from_str(include_str!(
        "../../../examples/fixtures/autocomplete-request.v1.json"
    ))
    .expect("fixture request")
}

fn write_executable(path: &Path, script: &str) {
    fs::write(path, script).expect("write fake executable");
    let mut permissions = fs::metadata(path).expect("fake metadata").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
    }
    fs::set_permissions(path, permissions).expect("chmod fake executable");
}

#[tokio::test]
async fn pi_provider_invokes_pi_with_isolation_flags_and_parses_jsonl_success() {
    let temp = TempDir::new().expect("tempdir");
    let args_log = temp.path().join("args.log");
    let fake_pi = temp.path().join("fake-pi");
    write_executable(
        &fake_pi,
        &format!(
            r#"#!/usr/bin/env bash
printf '%s\n' "$@" > '{}'
cat <<'JSONL'
{{"type":"message","role":"assistant","content":"{{\"insert_text\":\"\\\"Ada\\\")\",\"confidence\":0.82}}"}}
JSONL
"#,
            args_log.display()
        ),
    );

    let provider = PiProvider::new(PiProviderConfig {
        command: fake_pi,
        model: "test-model".to_owned(),
        timeout: Duration::from_secs(2),
        ..PiProviderConfig::default()
    });

    let output = provider
        .complete(fixture_request(), CancellationToken::new())
        .await
        .expect("provider success");

    let ProviderOutput::Candidate(candidate) = output else {
        panic!("expected candidate, got {output:?}");
    };
    assert_eq!(candidate.insert_text, "\"Ada\")");
    assert_eq!(candidate.confidence, 0.82);
    assert_eq!(candidate.source, "pi:test-model");

    let args = fs::read_to_string(args_log).expect("args log");
    for required in [
        "--no-tools",
        "--no-session",
        "--no-context-files",
        "--mode",
        "json",
        "--thinking",
        "minimal",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
    ] {
        assert!(
            args.lines().any(|line| line == required),
            "missing {required} in {args}"
        );
    }
}

#[tokio::test]
async fn pi_provider_repairs_malformed_assistant_json_with_one_retry() {
    let temp = TempDir::new().expect("tempdir");
    let count_file = temp.path().join("count");
    let fake_pi = temp.path().join("fake-pi");
    write_executable(
        &fake_pi,
        &format!(
            r#"#!/usr/bin/env bash
count_file='{}'
count=0
if [ -f "$count_file" ]; then count=$(cat "$count_file"); fi
count=$((count + 1))
echo "$count" > "$count_file"
if [ "$count" -eq 1 ]; then
  cat <<'JSONL'
{{"type":"message","role":"assistant","content":"not json"}}
JSONL
else
  cat <<'JSONL'
{{"type":"message","role":"assistant","content":"{{\"insert_text\":\"repaired\",\"confidence\":0.5}}"}}
JSONL
fi
"#,
            count_file.display()
        ),
    );

    let provider = PiProvider::new(PiProviderConfig {
        command: fake_pi,
        model: "test-model".to_owned(),
        timeout: Duration::from_secs(2),
        repair_retry: true,
        ..PiProviderConfig::default()
    });

    let output = provider
        .complete(fixture_request(), CancellationToken::new())
        .await
        .expect("provider success after repair");

    let ProviderOutput::Candidate(candidate) = output else {
        panic!("expected candidate, got {output:?}");
    };
    assert_eq!(candidate.insert_text, "repaired");
    assert_eq!(fs::read_to_string(count_file).expect("count"), "2\n");
}

#[tokio::test]
async fn pi_provider_times_out_slow_subprocesses() {
    let temp = TempDir::new().expect("tempdir");
    let fake_pi = temp.path().join("fake-pi");
    write_executable(
        &fake_pi,
        r#"#!/usr/bin/env bash
exec sleep 5
"#,
    );

    let provider = PiProvider::new(PiProviderConfig {
        command: fake_pi,
        model: "test-model".to_owned(),
        timeout: Duration::from_millis(50),
        ..PiProviderConfig::default()
    });

    let error = provider
        .complete(fixture_request(), CancellationToken::new())
        .await
        .expect_err("provider should time out");

    assert!(matches!(error, ProviderError::Timeout));
}

#[tokio::test]
async fn pi_provider_cancels_slow_subprocesses() {
    let temp = TempDir::new().expect("tempdir");
    let fake_pi = temp.path().join("fake-pi");
    write_executable(
        &fake_pi,
        r#"#!/usr/bin/env bash
exec sleep 5
"#,
    );

    let provider = PiProvider::new(PiProviderConfig {
        command: fake_pi,
        model: "test-model".to_owned(),
        timeout: Duration::from_secs(5),
        ..PiProviderConfig::default()
    });
    let token = CancellationToken::new();
    let cancel = token.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
    });

    let error = provider
        .complete(fixture_request(), token)
        .await
        .expect_err("provider should be cancelled");
    canceller.await.expect("canceller finished");

    assert!(matches!(error, ProviderError::Cancelled));
}

#[tokio::test]
async fn pi_provider_redacts_secrets_from_failure_diagnostics() {
    let temp = TempDir::new().expect("tempdir");
    let fake_pi = temp.path().join("fake-pi");
    write_executable(
        &fake_pi,
        r#"#!/usr/bin/env bash
echo 'token sk-SECRET1234567890' >&1
echo 'Authorization: Bearer abcdefghijklmnopqrstuvwxyz' >&2
echo '/home/abhmul/.config/pi/auth.json' >&2
exit 7
"#,
    );

    let provider = PiProvider::new(PiProviderConfig {
        command: fake_pi,
        model: "test-model".to_owned(),
        timeout: Duration::from_secs(2),
        ..PiProviderConfig::default()
    });

    let error = provider
        .complete(fixture_request(), CancellationToken::new())
        .await
        .expect_err("provider should fail");
    let diagnostics = error.diagnostics().expect("diagnostics");
    let combined = format!("{}\n{}", diagnostics.stdout, diagnostics.stderr);

    assert!(!combined.contains("sk-SECRET"), "{combined}");
    assert!(!combined.contains("Bearer abc"), "{combined}");
    assert!(!combined.contains("/home/abhmul/.config/pi"), "{combined}");
    assert!(combined.contains("[REDACTED]"), "{combined}");
}

#[test]
fn secret_redactor_redacts_common_secret_shapes() {
    let redacted = SecretRedactor::default().redact(
        "Authorization: Bearer abcdefghijklmnop\napi_key=sk-1234567890abcdef\n/home/me/.config/pi/auth.json",
    );

    assert!(!redacted.contains("abcdefghijklmnop"));
    assert!(!redacted.contains("sk-123456"));
    assert!(!redacted.contains("/home/me/.config/pi"));
}
