//! Built-in addon registries for the autocomplete broker.
//!
//! MVP addons are deliberately local and deterministic: they only transform request-supplied context, choose a prompt template, enforce broker privacy policy, and build the standard postprocessor pipeline.

use autocomplete_core::PostprocessorPipeline;
use autocomplete_protocol::{AutocompleteRequest, ErrorCode};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonSettings {
    pub prefix_chars: usize,
    pub suffix_chars: usize,
    pub remote_context_byte_limit: usize,
    pub excluded_globs: Vec<String>,
}

impl Default for AddonSettings {
    fn default() -> Self {
        Self {
            prefix_chars: 3_500,
            suffix_chars: 1_200,
            remote_context_byte_limit: 6_000,
            excluded_globs: vec![
                "**/.env*".to_owned(),
                "**/secrets/**".to_owned(),
                "**/prompt-buffer.md".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct AddonRuntime {
    context_providers: ContextProviderRegistry,
    prompt_templates: PromptTemplateRegistry,
    postprocessors: PostprocessorRegistry,
    policy: PolicyEngine,
}

impl AddonRuntime {
    pub fn new(settings: AddonSettings) -> Result<Self, AddonError> {
        Ok(Self {
            context_providers: ContextProviderRegistry::default_for(
                settings.prefix_chars,
                settings.suffix_chars,
            ),
            prompt_templates: PromptTemplateRegistry::default(),
            postprocessors: PostprocessorRegistry::default(),
            policy: PolicyEngine::new(settings.remote_context_byte_limit, settings.excluded_globs)?,
        })
    }

    pub fn prepare(&self, request: AutocompleteRequest) -> Result<PreparedRequest, AddonError> {
        let request = self.context_providers.apply(request)?;
        self.policy.enforce(&request)?;
        let prompt = self.prompt_templates.select(&request);
        Ok(PreparedRequest { request, prompt })
    }

    pub fn postprocessor_pipeline(&self) -> PostprocessorPipeline {
        self.postprocessors.pipeline()
    }
}

impl Default for AddonRuntime {
    fn default() -> Self {
        Self::new(AddonSettings::default()).expect("default addon settings must be valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    pub request: AutocompleteRequest,
    pub prompt: SelectedPrompt,
}

#[derive(Debug, Clone)]
pub struct ContextProviderRegistry {
    providers: Vec<Arc<dyn ContextProvider>>,
}

impl ContextProviderRegistry {
    pub fn new(providers: Vec<Arc<dyn ContextProvider>>) -> Self {
        Self { providers }
    }

    pub fn default_for(prefix_chars: usize, suffix_chars: usize) -> Self {
        Self::new(vec![Arc::new(NearbyTextWindow {
            prefix_chars,
            suffix_chars,
        })])
    }

    pub fn apply(
        &self,
        mut request: AutocompleteRequest,
    ) -> Result<AutocompleteRequest, AddonError> {
        for provider in &self.providers {
            request = provider.apply(request)?;
        }
        Ok(request)
    }
}

pub trait ContextProvider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn apply(&self, request: AutocompleteRequest) -> Result<AutocompleteRequest, AddonError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearbyTextWindow {
    pub prefix_chars: usize,
    pub suffix_chars: usize,
}

impl ContextProvider for NearbyTextWindow {
    fn name(&self) -> &'static str {
        "nearby_text_window"
    }

    fn apply(&self, mut request: AutocompleteRequest) -> Result<AutocompleteRequest, AddonError> {
        request.context.prefix = take_last_chars(&request.context.prefix, self.prefix_chars);
        request.context.suffix = take_first_chars(&request.context.suffix, self.suffix_chars);
        Ok(request)
    }
}

#[derive(Debug, Clone)]
pub struct PromptTemplateRegistry {
    templates: Vec<Arc<dyn PromptTemplate>>,
}

impl PromptTemplateRegistry {
    pub fn new(templates: Vec<Arc<dyn PromptTemplate>>) -> Self {
        Self { templates }
    }

    pub fn select(&self, request: &AutocompleteRequest) -> SelectedPrompt {
        self.templates
            .iter()
            .find(|template| template.matches(request))
            .map(|template| template.render(request))
            .unwrap_or_else(|| CodePromptTemplate.render(request))
    }
}

impl Default for PromptTemplateRegistry {
    fn default() -> Self {
        Self::new(vec![
            Arc::new(MarkdownPromptTemplate),
            Arc::new(CodePromptTemplate),
        ])
    }
}

pub trait PromptTemplate: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn matches(&self, request: &AutocompleteRequest) -> bool;
    fn render(&self, request: &AutocompleteRequest) -> SelectedPrompt;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedPrompt {
    pub name: String,
    pub kind: PromptKind,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Code,
    Markdown,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CodePromptTemplate;

impl PromptTemplate for CodePromptTemplate {
    fn name(&self) -> &'static str {
        "code_inline_completion"
    }

    fn matches(&self, request: &AutocompleteRequest) -> bool {
        !is_markdown_language(&request.document.language_id)
    }

    fn render(&self, _request: &AutocompleteRequest) -> SelectedPrompt {
        SelectedPrompt {
            name: self.name().to_owned(),
            kind: PromptKind::Code,
            system_prompt: "Complete code at the cursor. Return only insertable code, without markdown fences, explanations, or repeated prefix text.".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownPromptTemplate;

impl PromptTemplate for MarkdownPromptTemplate {
    fn name(&self) -> &'static str {
        "markdown_prose_completion"
    }

    fn matches(&self, request: &AutocompleteRequest) -> bool {
        is_markdown_language(&request.document.language_id)
    }

    fn render(&self, _request: &AutocompleteRequest) -> SelectedPrompt {
        SelectedPrompt {
            name: self.name().to_owned(),
            kind: PromptKind::Markdown,
            system_prompt: "Continue markdown prose at the cursor. Return only text to insert, preserving note style and avoiding explanations.".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PostprocessorRegistry;

impl PostprocessorRegistry {
    pub fn pipeline(&self) -> PostprocessorPipeline {
        PostprocessorPipeline::default()
    }
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    remote_context_byte_limit: usize,
    excluded_globs: Vec<String>,
    excluded_set: GlobSet,
}

impl PolicyEngine {
    pub fn new(
        remote_context_byte_limit: usize,
        excluded_globs: Vec<String>,
    ) -> Result<Self, AddonError> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &excluded_globs {
            builder.add(
                Glob::new(pattern).map_err(|source| AddonError::InvalidGlob {
                    pattern: pattern.clone(),
                    source,
                })?,
            );
        }
        let excluded_set = builder.build().map_err(AddonError::GlobBuild)?;
        Ok(Self {
            remote_context_byte_limit,
            excluded_globs,
            excluded_set,
        })
    }

    pub fn remote_context_byte_limit(&self) -> usize {
        self.remote_context_byte_limit
    }

    pub fn excluded_globs(&self) -> &[String] {
        &self.excluded_globs
    }

    pub fn enforce(&self, request: &AutocompleteRequest) -> Result<(), AddonError> {
        if self.uri_is_excluded(&request.document.uri) {
            return Err(AddonError::ExcludedPath {
                uri: request.document.uri.clone(),
            });
        }

        let context_bytes = request.context.prefix.len()
            + request.context.suffix.len()
            + request.context.selected_text.len();
        if context_bytes > self.remote_context_byte_limit {
            return Err(AddonError::RemoteContextTooLarge {
                limit: self.remote_context_byte_limit,
                actual: context_bytes,
            });
        }

        Ok(())
    }

    fn uri_is_excluded(&self, uri: &str) -> bool {
        uri_match_candidates(uri)
            .iter()
            .any(|candidate| self.excluded_set.is_match(candidate))
    }
}

#[derive(Debug, Error)]
pub enum AddonError {
    #[error("invalid exclusion glob {pattern:?}: {source}")]
    InvalidGlob {
        pattern: String,
        source: globset::Error,
    },
    #[error("failed to build exclusion glob set: {0}")]
    GlobBuild(globset::Error),
    #[error("document path is excluded by broker privacy policy: {uri}")]
    ExcludedPath { uri: String },
    #[error(
        "request context is {actual} bytes after broker context processing, exceeding remote limit {limit}"
    )]
    RemoteContextTooLarge { limit: usize, actual: usize },
}

impl AddonError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::RemoteContextTooLarge { .. } => ErrorCode::ContextTooLarge,
            Self::InvalidGlob { .. } | Self::GlobBuild(_) => ErrorCode::InternalError,
            Self::ExcludedPath { .. } => ErrorCode::InvalidRequest,
        }
    }
}

fn is_markdown_language(language_id: &str) -> bool {
    matches!(
        language_id.to_ascii_lowercase().as_str(),
        "markdown" | "md" | "mdx"
    )
}

fn take_last_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_owned();
    }
    text.chars().skip(total - max_chars).collect()
}

fn take_first_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn uri_match_candidates(uri: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    candidates.push(uri.to_owned());

    if let Ok(parsed) = Url::parse(uri)
        && parsed.scheme() == "file"
        && let Ok(path) = parsed.to_file_path()
    {
        add_path_candidates(&mut candidates, &path);
    }

    add_stripped_leading_slash_candidates(&mut candidates);
    candidates.sort();
    candidates.dedup();
    candidates
}

fn add_path_candidates(candidates: &mut Vec<String>, path: &Path) {
    let display = path.to_string_lossy().into_owned();
    candidates.push(display.clone());
    if let Some(stripped) = display.strip_prefix('/') {
        candidates.push(stripped.to_owned());
    }
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        candidates.push(file_name.to_owned());
    }
}

fn add_stripped_leading_slash_candidates(candidates: &mut Vec<String>) {
    let stripped = candidates
        .iter()
        .filter_map(|candidate| candidate.strip_prefix('/').map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    candidates.extend(stripped);
}

#[cfg(test)]
mod tests {
    use super::{take_first_chars, take_last_chars};

    #[test]
    fn context_trimming_respects_char_boundaries() {
        assert_eq!(take_last_chars("αβγ", 2), "βγ");
        assert_eq!(take_first_chars("αβγ", 2), "αβ");
    }
}
