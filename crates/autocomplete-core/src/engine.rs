use crate::{
    CompletionProvider, PostprocessError, PostprocessorPipeline, ProviderError, ProviderOutput,
};
use autocomplete_protocol::{
    AutocompleteRequest, AutocompleteResponse, ErrorCode, PROTOCOL_VERSION, ProtocolError,
    ResponseMetadata, Validate,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub struct AutocompleteEngine {
    provider: Arc<dyn CompletionProvider>,
    postprocessors: PostprocessorPipeline,
}

impl AutocompleteEngine {
    pub fn new<P>(provider: P, postprocessors: PostprocessorPipeline) -> Self
    where
        P: CompletionProvider + 'static,
    {
        Self {
            provider: Arc::new(provider),
            postprocessors,
        }
    }

    pub fn from_provider_arc(
        provider: Arc<dyn CompletionProvider>,
        postprocessors: PostprocessorPipeline,
    ) -> Self {
        Self {
            provider,
            postprocessors,
        }
    }

    pub async fn complete(
        &self,
        request: AutocompleteRequest,
        cancellation: CancellationToken,
    ) -> AutocompleteResponse {
        let started = Instant::now();
        let request_id = request.request_id;

        if let Err(errors) = request.validate() {
            return AutocompleteResponse::Error {
                protocol_version: PROTOCOL_VERSION,
                request_id,
                error: ProtocolError {
                    code: ErrorCode::InvalidRequest,
                    message: errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                },
                metadata: Some(metadata(started, Duration::ZERO, false)),
            };
        }

        if cancellation.is_cancelled() {
            return cancelled_response(request_id, started, Duration::ZERO);
        }

        let provider_started = Instant::now();
        let provider_token = cancellation.child_token();
        let deadline = Duration::from_millis(request.options.deadline_ms);
        let provider = Arc::clone(&self.provider);
        let provider_future = provider.complete(request.clone(), provider_token.clone());

        let provider_result = tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                provider_token.cancel();
                Err(ProviderError::Cancelled)
            }
            result = tokio::time::timeout(deadline, provider_future) => {
                match result {
                    Ok(provider_result) => provider_result,
                    Err(_) => {
                        provider_token.cancel();
                        Err(ProviderError::Timeout)
                    }
                }
            }
        };
        let provider_latency = provider_started.elapsed();

        match provider_result {
            Ok(ProviderOutput::Candidate(candidate)) => {
                match self.postprocessors.process(&request, candidate.insert_text) {
                    Ok(insert_text) if !insert_text.trim().is_empty() => AutocompleteResponse::Ok {
                        protocol_version: PROTOCOL_VERSION,
                        request_id,
                        insert_text,
                        confidence: candidate.confidence.clamp(0.0, 1.0),
                        source: candidate.source,
                        metadata: Some(metadata(started, provider_latency, true)),
                    },
                    Ok(_)
                    | Err(PostprocessError::Empty)
                    | Err(PostprocessError::Rejected { .. }) => {
                        AutocompleteResponse::NoSuggestion {
                            protocol_version: PROTOCOL_VERSION,
                            request_id,
                            metadata: Some(metadata(started, provider_latency, true)),
                        }
                    }
                }
            }
            Ok(ProviderOutput::NoSuggestion) => AutocompleteResponse::NoSuggestion {
                protocol_version: PROTOCOL_VERSION,
                request_id,
                metadata: Some(metadata(started, provider_latency, false)),
            },
            Err(ProviderError::Cancelled) => {
                cancelled_response(request_id, started, provider_latency)
            }
            Err(ProviderError::Timeout) => AutocompleteResponse::Error {
                protocol_version: PROTOCOL_VERSION,
                request_id,
                error: ProtocolError {
                    code: ErrorCode::ProviderTimeout,
                    message: "Autocomplete provider exceeded deadline.".to_owned(),
                },
                metadata: Some(metadata(started, provider_latency, false)),
            },
            Err(ProviderError::MalformedOutput { message, .. }) => AutocompleteResponse::Error {
                protocol_version: PROTOCOL_VERSION,
                request_id,
                error: ProtocolError {
                    code: ErrorCode::ProviderMalformedOutput,
                    message,
                },
                metadata: Some(metadata(started, provider_latency, false)),
            },
            Err(ProviderError::Failed { message, .. }) => AutocompleteResponse::Error {
                protocol_version: PROTOCOL_VERSION,
                request_id,
                error: ProtocolError {
                    code: ErrorCode::ProviderError,
                    message,
                },
                metadata: Some(metadata(started, provider_latency, false)),
            },
        }
    }
}

fn cancelled_response(
    request_id: uuid::Uuid,
    started: Instant,
    provider_latency: Duration,
) -> AutocompleteResponse {
    AutocompleteResponse::Cancelled {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        metadata: Some(metadata(started, provider_latency, false)),
    }
}

fn metadata(started: Instant, provider_latency: Duration, postprocessed: bool) -> ResponseMetadata {
    ResponseMetadata {
        latency_ms: millis_saturating(started.elapsed()),
        provider_latency_ms: millis_saturating(provider_latency),
        postprocessed,
    }
}

fn millis_saturating(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
