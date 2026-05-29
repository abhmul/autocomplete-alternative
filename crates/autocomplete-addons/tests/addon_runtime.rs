use autocomplete_addons::{AddonRuntime, AddonSettings, PromptKind};
use autocomplete_protocol::AutocompleteRequest;

fn fixture_request() -> AutocompleteRequest {
    serde_json::from_str(include_str!(
        "../../../examples/fixtures/autocomplete-request.v1.json"
    ))
    .expect("fixture request")
}

#[test]
fn addon_runtime_trims_context_selects_code_prompt_and_rejects_excluded_paths() {
    let mut request = fixture_request();
    request.context.prefix = "abcdef".to_owned();
    request.context.suffix = "uvwxyz".to_owned();

    let settings = AddonSettings {
        prefix_chars: 3,
        suffix_chars: 2,
        remote_context_byte_limit: 16,
        excluded_globs: vec!["**/.env*".to_owned()],
    };
    let runtime = AddonRuntime::new(settings).expect("runtime");

    let prepared = runtime.prepare(request).expect("prepared request");
    assert_eq!(prepared.request.context.prefix, "def");
    assert_eq!(prepared.request.context.suffix, "uv");
    assert_eq!(prepared.prompt.kind, PromptKind::Code);

    let mut excluded = fixture_request();
    excluded.document.uri = "file:///repo/.env.local".to_owned();
    assert!(runtime.prepare(excluded).is_err());
}

#[test]
fn addon_runtime_selects_markdown_prompt_for_obsidian_notes() {
    let mut request = fixture_request();
    request.client.name = "obsidian".to_owned();
    request.document.uri = "file:///vault/daily.md".to_owned();
    request.document.language_id = "markdown".to_owned();

    let prepared = AddonRuntime::default()
        .prepare(request)
        .expect("prepared request");

    assert_eq!(prepared.prompt.kind, PromptKind::Markdown);
    assert!(prepared.prompt.system_prompt.contains("prose"));
}
