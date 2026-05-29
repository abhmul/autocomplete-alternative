//! HTTP server for the local autocomplete broker.

use autocomplete_addons::{AddonError, AddonRuntime, AddonSettings};
use autocomplete_core::{
    AutocompleteEngine, CompletionProvider, MockProvider, ProviderRequestContext,
};
use autocomplete_protocol::{
    AutocompleteRequest, AutocompleteResponse, CancelResponse, CancelStatus, ErrorCode,
    HealthResponse, HealthStatus, PROTOCOL_VERSION, ProtocolError, ProviderHealth, ProviderStatus,
    ReloadResponse, ReloadStatus, Validate,
};
use autocomplete_provider_pi::{PiProvider, PiProviderConfig};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/autocomplete", post(autocomplete))
        .route("/v1/cancel/{request_id}", post(cancel))
        .route("/v1/reload", post(reload))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrokerConfig {
    pub bind_addr: String,
    pub provider: ProviderConfig,
    pub mock: MockProviderConfig,
    pub pi: PiProviderSettings,
    pub trigger: TriggerConfig,
    pub context: ContextConfig,
    pub privacy: PrivacyConfig,
}

impl BrokerConfig {
    pub fn load(path: impl AsRef<FsPath>) -> Result<Self, ServerError> {
        let settings = config::Config::builder()
            .add_source(
                config::File::from(path.as_ref())
                    .format(config::FileFormat::Toml)
                    .required(true),
            )
            .build()?;
        let config = settings.try_deserialize::<Self>()?;
        config.validate()?;
        Ok(config)
    }

    pub fn bind_socket_addr(&self) -> Result<SocketAddr, ServerError> {
        self.bind_addr
            .parse()
            .map_err(|source| ServerError::InvalidBindAddress {
                bind_addr: self.bind_addr.clone(),
                source,
            })
    }

    pub fn addon_settings(&self) -> AddonSettings {
        AddonSettings {
            prefix_chars: self.context.prefix_chars,
            suffix_chars: self.context.suffix_chars,
            remote_context_byte_limit: self.privacy.remote_context_byte_limit,
            excluded_globs: self.privacy.excluded_globs.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), ServerError> {
        if self.context.prefix_chars + self.context.suffix_chars == 0 {
            return Err(ServerError::InvalidConfig(
                "context prefix_chars and suffix_chars cannot both be zero".to_owned(),
            ));
        }
        if self.privacy.remote_context_byte_limit == 0 {
            return Err(ServerError::InvalidConfig(
                "privacy remote_context_byte_limit must be greater than zero".to_owned(),
            ));
        }
        if self.trigger.idle_delay_ms == 0 {
            return Err(ServerError::InvalidConfig(
                "trigger idle_delay_ms must be greater than zero".to_owned(),
            ));
        }
        AddonRuntime::new(self.addon_settings())?;
        Ok(())
    }
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:32145".to_owned(),
            provider: ProviderConfig::default(),
            mock: MockProviderConfig::default(),
            pi: PiProviderSettings::default(),
            trigger: TriggerConfig::default(),
            context: ContextConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::Mock,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Mock,
    Pi,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MockProviderConfig {
    pub insert_text: String,
    pub confidence: f64,
    pub delay_ms: u64,
}

impl Default for MockProviderConfig {
    fn default() -> Self {
        Self {
            insert_text: "mock completion".to_owned(),
            confidence: 1.0,
            delay_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PiProviderSettings {
    pub command: PathBuf,
    pub provider: Option<String>,
    pub model: String,
    pub thinking: String,
    pub timeout_ms: u64,
    pub repair_retry: bool,
}

impl PiProviderSettings {
    fn to_provider_config(&self) -> PiProviderConfig {
        PiProviderConfig {
            command: self.command.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            thinking: self.thinking.clone(),
            timeout: Duration::from_millis(self.timeout_ms),
            repair_retry: self.repair_retry,
        }
    }
}

impl Default for PiProviderSettings {
    fn default() -> Self {
        let provider = PiProviderConfig::default();
        Self {
            command: provider.command,
            provider: provider.provider,
            model: provider.model,
            thinking: provider.thinking,
            timeout_ms: duration_millis(provider.timeout),
            repair_retry: provider.repair_retry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TriggerConfig {
    pub mode: TriggerMode,
    pub idle_delay_ms: u64,
    pub min_prefix_chars: usize,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            mode: TriggerMode::IdleOrManual,
            idle_delay_ms: 500,
            min_prefix_chars: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    IdleOrManual,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContextConfig {
    pub prefix_chars: usize,
    pub suffix_chars: usize,
    pub include_open_files: bool,
    pub include_workspace_symbols: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            prefix_chars: 3_500,
            suffix_chars: 1_200,
            include_open_files: false,
            include_workspace_symbols: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PrivacyConfig {
    pub remote_context_byte_limit: usize,
    pub excluded_globs: Vec<String>,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            remote_context_byte_limit: 6_000,
            excluded_globs: vec![
                "**/.env*".to_owned(),
                "**/secrets/**".to_owned(),
                "**/prompt-buffer.md".to_owned(),
            ],
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    runtime: RwLock<BrokerRuntime>,
    inflight: Mutex<HashMap<Uuid, CancellationToken>>,
    completed: Mutex<HashSet<Uuid>>,
    config_path: Option<PathBuf>,
}

impl AppState {
    pub fn from_config(config: BrokerConfig) -> Result<Self, ServerError> {
        config.validate()?;
        Ok(Self {
            inner: Arc::new(AppStateInner {
                runtime: RwLock::new(BrokerRuntime::from_config(config)?),
                inflight: Mutex::new(HashMap::new()),
                completed: Mutex::new(HashSet::new()),
                config_path: None,
            }),
        })
    }

    pub fn from_config_path(path: impl Into<PathBuf>) -> Result<Self, ServerError> {
        let path = path.into();
        let config = BrokerConfig::load(&path)?;
        Ok(Self {
            inner: Arc::new(AppStateInner {
                runtime: RwLock::new(BrokerRuntime::from_config(config)?),
                inflight: Mutex::new(HashMap::new()),
                completed: Mutex::new(HashSet::new()),
                config_path: Some(path),
            }),
        })
    }

    async fn snapshot(&self) -> RuntimeSnapshot {
        self.inner.runtime.read().await.snapshot()
    }

    async fn start_request(&self, request_id: Uuid) -> CancellationToken {
        let token = CancellationToken::new();
        let mut inflight = self.inner.inflight.lock().await;
        if let Some(previous) = inflight.insert(request_id, token.clone()) {
            previous.cancel();
        }
        self.inner.completed.lock().await.remove(&request_id);
        token
    }

    async fn finish_request(&self, request_id: Uuid) {
        self.inner.inflight.lock().await.remove(&request_id);
        self.inner.completed.lock().await.insert(request_id);
    }

    async fn cancel_request(&self, request_id: Uuid) -> CancelStatus {
        if let Some(token) = self.inner.inflight.lock().await.get(&request_id).cloned() {
            token.cancel();
            return CancelStatus::Cancelled;
        }
        if self.inner.completed.lock().await.contains(&request_id) {
            return CancelStatus::AlreadyCompleted;
        }
        CancelStatus::NotFound
    }

    async fn reload_config(&self) -> Result<ReloadStatus, ServerError> {
        let Some(path) = &self.inner.config_path else {
            return Ok(ReloadStatus::Unchanged);
        };
        let config = BrokerConfig::load(path)?;
        let runtime = BrokerRuntime::from_config(config)?;
        *self.inner.runtime.write().await = runtime;
        Ok(ReloadStatus::Reloaded)
    }
}

struct BrokerRuntime {
    config: BrokerConfig,
    provider_name: String,
    provider_status: ProviderStatus,
    engine: Arc<AutocompleteEngine>,
    addons: AddonRuntime,
}

impl BrokerRuntime {
    fn from_config(config: BrokerConfig) -> Result<Self, ServerError> {
        let addons = AddonRuntime::new(config.addon_settings())?;
        let (provider_name, provider_status, provider) = provider_from_config(&config);
        let engine = Arc::new(AutocompleteEngine::from_provider_arc(
            provider,
            addons.postprocessor_pipeline(),
        ));
        Ok(Self {
            config,
            provider_name,
            provider_status,
            engine,
            addons,
        })
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            provider_name: self.provider_name.clone(),
            provider_status: self.provider_status,
            engine: Arc::clone(&self.engine),
            addons: self.addons.clone(),
        }
    }
}

#[derive(Clone)]
struct RuntimeSnapshot {
    provider_name: String,
    provider_status: ProviderStatus,
    engine: Arc<AutocompleteEngine>,
    addons: AddonRuntime,
}

fn provider_from_config(
    config: &BrokerConfig,
) -> (String, ProviderStatus, Arc<dyn CompletionProvider>) {
    match config.provider.kind {
        ProviderKind::Mock => {
            let mut provider = MockProvider::new(config.mock.insert_text.clone())
                .with_confidence(config.mock.confidence)
                .with_source("mock");
            if config.mock.delay_ms > 0 {
                provider = provider.with_delay(Duration::from_millis(config.mock.delay_ms));
            }
            (
                "mock".to_owned(),
                ProviderStatus::Available,
                Arc::new(provider),
            )
        }
        ProviderKind::Pi => {
            let provider_config = config.pi.to_provider_config();
            let provider_name = format!("pi:{}", provider_config.model);
            (
                provider_name,
                ProviderStatus::Unknown,
                Arc::new(PiProvider::new(provider_config)),
            )
        }
    }
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let runtime = state.snapshot().await;
    Json(HealthResponse {
        protocol_version: PROTOCOL_VERSION,
        status: HealthStatus::Ok,
        provider: ProviderHealth {
            name: runtime.provider_name,
            status: runtime.provider_status,
        },
    })
}

async fn autocomplete(
    State(state): State<AppState>,
    body: Bytes,
) -> (StatusCode, Json<AutocompleteResponse>) {
    let request = match serde_json::from_slice::<AutocompleteRequest>(&body) {
        Ok(request) => request,
        Err(error) => {
            return protocol_error_response(
                StatusCode::BAD_REQUEST,
                Uuid::nil(),
                ErrorCode::InvalidRequest,
                format!("invalid autocomplete request JSON: {error}"),
            );
        }
    };

    if let Err(errors) = request.validate() {
        let code = if request.protocol_version != PROTOCOL_VERSION {
            ErrorCode::UnsupportedProtocolVersion
        } else {
            ErrorCode::InvalidRequest
        };
        return protocol_error_response(
            StatusCode::BAD_REQUEST,
            request.request_id,
            code,
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        );
    }

    let request_id = request.request_id;
    let snapshot = state.snapshot().await;
    let prepared = match snapshot.addons.prepare(request) {
        Ok(prepared) => prepared,
        Err(error) => {
            return protocol_error_response(
                addon_error_status(&error),
                request_id,
                error.error_code(),
                error.to_string(),
            );
        }
    };
    let request_id = prepared.request.request_id;
    tracing::debug!(prompt = %prepared.prompt.name, prompt_kind = ?prepared.prompt.kind, "selected autocomplete prompt template");
    let provider_context =
        ProviderRequestContext::with_prompt(prepared.prompt.name, prepared.prompt.system_prompt);

    let cancellation = state.start_request(request_id).await;
    let response = snapshot
        .engine
        .complete_with_provider_context(prepared.request, provider_context, cancellation)
        .await;
    state.finish_request(request_id).await;
    (StatusCode::OK, Json(response))
}

async fn cancel(
    State(state): State<AppState>,
    Path(request_id): Path<Uuid>,
) -> Json<CancelResponse> {
    let status = state.cancel_request(request_id).await;
    Json(CancelResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        status,
    })
}

async fn reload(State(state): State<AppState>) -> (StatusCode, Json<ReloadResponse>) {
    let status = match state.reload_config().await {
        Ok(status) => status,
        Err(error) => {
            tracing::warn!(%error, "broker reload failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReloadResponse {
                    protocol_version: PROTOCOL_VERSION,
                    status: ReloadStatus::Unchanged,
                }),
            );
        }
    };
    (
        StatusCode::OK,
        Json(ReloadResponse {
            protocol_version: PROTOCOL_VERSION,
            status,
        }),
    )
}

pub async fn run(config_path: Option<PathBuf>) -> Result<(), ServerError> {
    let state = match config_path {
        Some(path) => AppState::from_config_path(path)?,
        None => AppState::from_config(BrokerConfig::default())?,
    };
    let bind_addr = state.inner.runtime.read().await.config.bind_socket_addr()?;
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(%bind_addr, "autocomplete broker listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}

fn protocol_error_response(
    status: StatusCode,
    request_id: Uuid,
    code: ErrorCode,
    message: String,
) -> (StatusCode, Json<AutocompleteResponse>) {
    (
        status,
        Json(AutocompleteResponse::Error {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            error: ProtocolError { code, message },
            metadata: None,
        }),
    )
}

fn addon_error_status(error: &AddonError) -> StatusCode {
    match error {
        AddonError::RemoteContextTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        AddonError::ExcludedPath { .. } => StatusCode::BAD_REQUEST,
        AddonError::InvalidGlob { .. } | AddonError::GlobBuild(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("addon error: {0}")]
    Addon(#[from] AddonError),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("invalid bind address {bind_addr:?}: {source}")]
    InvalidBindAddress {
        bind_addr: String,
        source: std::net::AddrParseError,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
