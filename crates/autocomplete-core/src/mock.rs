use crate::{
    CompletionCandidate, CompletionProvider, ProviderError, ProviderOutput, ProviderRequestContext,
    ProviderResult,
};
use async_trait::async_trait;
use autocomplete_protocol::AutocompleteRequest;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct MockProvider {
    insert_text: String,
    confidence: f64,
    source: String,
    delay: Option<Duration>,
}

impl MockProvider {
    pub fn new(insert_text: impl Into<String>) -> Self {
        Self {
            insert_text: insert_text.into(),
            confidence: 1.0,
            source: "mock".to_owned(),
            delay: None,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new("mock completion")
    }
}

#[async_trait]
impl CompletionProvider for MockProvider {
    async fn complete_with_context(
        &self,
        _request: AutocompleteRequest,
        _provider_context: ProviderRequestContext,
        cancellation: CancellationToken,
    ) -> ProviderResult {
        if cancellation.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }

        if let Some(delay) = self.delay {
            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = cancellation.cancelled() => return Err(ProviderError::Cancelled),
            }
        }

        if cancellation.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }

        Ok(ProviderOutput::Candidate(CompletionCandidate::new(
            self.insert_text.clone(),
            self.confidence,
            self.source.clone(),
        )))
    }
}
