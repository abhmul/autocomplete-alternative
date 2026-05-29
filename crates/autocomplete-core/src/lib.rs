//! Core autocomplete engine behavior.

mod engine;
mod mock;
mod postprocess;
mod provider;

pub use engine::AutocompleteEngine;
pub use mock::MockProvider;
pub use postprocess::{
    EnforceMaxChars, NormalizeNewlines, PostprocessContext, PostprocessError, Postprocessor,
    PostprocessorPipeline, RejectExplanations, RemoveRepeatedPrefix, StripMarkdownFences,
};
pub use provider::{
    CompletionCandidate, CompletionProvider, ProviderDiagnostics, ProviderError, ProviderOutput,
    ProviderRequestContext, ProviderResult,
};
