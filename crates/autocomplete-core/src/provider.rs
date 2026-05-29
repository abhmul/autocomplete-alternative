use async_trait::async_trait;
use autocomplete_protocol::AutocompleteRequest;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderRequestContext {
    pub prompt_name: Option<String>,
    pub system_prompt: Option<String>,
}

impl ProviderRequestContext {
    pub fn with_prompt(prompt_name: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            prompt_name: Some(prompt_name.into()),
            system_prompt: Some(system_prompt.into()),
        }
    }
}

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    async fn complete(
        &self,
        request: AutocompleteRequest,
        cancellation: CancellationToken,
    ) -> ProviderResult {
        self.complete_with_context(request, ProviderRequestContext::default(), cancellation)
            .await
    }

    async fn complete_with_context(
        &self,
        request: AutocompleteRequest,
        provider_context: ProviderRequestContext,
        cancellation: CancellationToken,
    ) -> ProviderResult;
}

pub type ProviderResult = Result<ProviderOutput, ProviderError>;

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderOutput {
    Candidate(CompletionCandidate),
    NoSuggestion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionCandidate {
    pub insert_text: String,
    pub confidence: f64,
    pub source: String,
}

impl CompletionCandidate {
    pub fn new(insert_text: impl Into<String>, confidence: f64, source: impl Into<String>) -> Self {
        Self {
            insert_text: insert_text.into(),
            confidence,
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderDiagnostics {
    pub stdout: String,
    pub stderr: String,
}

impl ProviderDiagnostics {
    pub fn is_empty(&self) -> bool {
        self.stdout.is_empty() && self.stderr.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    #[error("autocomplete provider exceeded deadline")]
    Timeout,
    #[error("autocomplete request was cancelled")]
    Cancelled,
    #[error("provider returned malformed output: {message}")]
    MalformedOutput {
        message: String,
        diagnostics: ProviderDiagnostics,
    },
    #[error("provider failed: {message}")]
    Failed {
        message: String,
        diagnostics: ProviderDiagnostics,
    },
}

impl ProviderError {
    pub fn malformed(message: impl Into<String>, diagnostics: ProviderDiagnostics) -> Self {
        Self::MalformedOutput {
            message: message.into(),
            diagnostics,
        }
    }

    pub fn failed(message: impl Into<String>, diagnostics: ProviderDiagnostics) -> Self {
        Self::Failed {
            message: message.into(),
            diagnostics,
        }
    }

    pub fn diagnostics(&self) -> Option<&ProviderDiagnostics> {
        match self {
            Self::MalformedOutput { diagnostics, .. } | Self::Failed { diagnostics, .. } => {
                Some(diagnostics)
            }
            Self::Timeout | Self::Cancelled => None,
        }
    }
}
