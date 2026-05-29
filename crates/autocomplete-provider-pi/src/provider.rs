use crate::{PiEventStreamParser, RenderedPrompt, SecretRedactor};
use async_trait::async_trait;
use autocomplete_core::{
    CompletionCandidate, CompletionProvider, ProviderDiagnostics, ProviderError, ProviderOutput,
    ProviderResult,
};
use autocomplete_protocol::AutocompleteRequest;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct PiProviderConfig {
    pub command: PathBuf,
    pub provider: Option<String>,
    pub model: String,
    pub thinking: String,
    pub timeout: Duration,
    pub repair_retry: bool,
}

impl Default for PiProviderConfig {
    fn default() -> Self {
        Self {
            command: PathBuf::from("pi"),
            provider: None,
            model: "openai/gpt-5.5".to_owned(),
            thinking: "minimal".to_owned(),
            timeout: Duration::from_millis(2_500),
            repair_retry: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PiProvider {
    config: PiProviderConfig,
    parser: PiEventStreamParser,
    redactor: SecretRedactor,
}

impl PiProvider {
    pub fn new(config: PiProviderConfig) -> Self {
        Self {
            config,
            parser: PiEventStreamParser::default(),
            redactor: SecretRedactor::default(),
        }
    }

    pub fn config(&self) -> &PiProviderConfig {
        &self.config
    }

    async fn run_completion_attempt(
        &self,
        prompt: RenderedPrompt,
        timeout: Duration,
        cancellation: CancellationToken,
    ) -> Result<AttemptOutput, ProviderError> {
        let output = self.run_pi(prompt, timeout, cancellation).await?;
        let assistant_text = self
            .parser
            .final_assistant_text(&output.stdout)
            .map_err(|error| ProviderError::malformed(error.to_string(), output.diagnostics()))?;
        let response = parse_provider_response(&assistant_text)
            .map_err(|error| ProviderError::malformed(error, output.diagnostics()))?;
        Ok(AttemptOutput { response })
    }

    async fn run_pi(
        &self,
        prompt: RenderedPrompt,
        timeout: Duration,
        cancellation: CancellationToken,
    ) -> Result<ProcessOutput, ProviderError> {
        if cancellation.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }

        let mut command = Command::new(&self.config.command);
        command
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for arg in self.command_args(&prompt) {
            command.arg(arg);
        }

        let mut child = command.spawn().map_err(|error| {
            ProviderError::failed(
                format!("failed to spawn pi subprocess: {error}"),
                ProviderDiagnostics::default(),
            )
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            ProviderError::failed(
                "failed to capture pi stdout",
                ProviderDiagnostics::default(),
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ProviderError::failed(
                "failed to capture pi stderr",
                ProviderDiagnostics::default(),
            )
        })?;
        let stdout_task = tokio::spawn(read_pipe(stdout));
        let stderr_task = tokio::spawn(read_pipe(stderr));

        let (status_result, interrupted) = tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                (Err(ProviderError::Cancelled), true)
            }
            () = tokio::time::sleep(timeout) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                (Err(ProviderError::Timeout), true)
            }
            status = child.wait() => {
                (status.map_err(|error| {
                    ProviderError::failed(
                        format!("failed while waiting for pi subprocess: {error}"),
                        ProviderDiagnostics::default(),
                    )
                }), false)
            }
        };

        let stdout = collect_pipe_task(stdout_task, interrupted).await;
        let stderr = collect_pipe_task(stderr_task, interrupted).await;
        let output = ProcessOutput {
            stdout,
            stderr,
            redactor: self.redactor.clone(),
        };

        let status = status_result?;
        if !status.success() {
            return Err(ProviderError::failed(
                format!("pi subprocess exited with {status}"),
                output.diagnostics(),
            ));
        }

        Ok(output)
    }

    fn command_args(&self, prompt: &RenderedPrompt) -> Vec<String> {
        let mut args = vec![
            "--print".to_owned(),
            "--mode".to_owned(),
            "json".to_owned(),
            "--no-tools".to_owned(),
            "--no-session".to_owned(),
            "--no-context-files".to_owned(),
            "--no-extensions".to_owned(),
            "--no-skills".to_owned(),
            "--no-prompt-templates".to_owned(),
            "--no-themes".to_owned(),
        ];
        if let Some(provider) = &self.config.provider {
            args.push("--provider".to_owned());
            args.push(provider.clone());
        }
        args.extend([
            "--model".to_owned(),
            self.config.model.clone(),
            "--thinking".to_owned(),
            self.config.thinking.clone(),
            "--system-prompt".to_owned(),
            prompt.system_prompt.clone(),
            prompt.request_json.clone(),
        ]);
        args
    }
}

impl Default for PiProvider {
    fn default() -> Self {
        Self::new(PiProviderConfig::default())
    }
}

#[async_trait]
impl CompletionProvider for PiProvider {
    async fn complete(
        &self,
        request: AutocompleteRequest,
        cancellation: CancellationToken,
    ) -> ProviderResult {
        let timeout = self
            .config
            .timeout
            .min(Duration::from_millis(request.options.deadline_ms));
        let prompt = RenderedPrompt::for_completion(&request)?;

        match self
            .run_completion_attempt(prompt.clone(), timeout, cancellation.clone())
            .await
        {
            Ok(output) => Ok(output.into_provider_output(&self.config.model)),
            Err(ProviderError::MalformedOutput {
                message,
                diagnostics,
            }) if self.config.repair_retry && !cancellation.is_cancelled() => {
                let repair_prompt = prompt.for_repair(&message);
                match self
                    .run_completion_attempt(repair_prompt, timeout, cancellation)
                    .await
                {
                    Ok(output) => Ok(output.into_provider_output(&self.config.model)),
                    Err(ProviderError::MalformedOutput { message, .. }) => {
                        Err(ProviderError::MalformedOutput {
                            message,
                            diagnostics,
                        })
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug, Clone)]
struct ProcessOutput {
    stdout: String,
    stderr: String,
    redactor: SecretRedactor,
}

impl ProcessOutput {
    fn diagnostics(&self) -> ProviderDiagnostics {
        ProviderDiagnostics {
            stdout: self.redactor.redact(&self.stdout),
            stderr: self.redactor.redact(&self.stderr),
        }
    }
}

#[derive(Debug, Clone)]
struct AttemptOutput {
    response: ProviderJsonResponse,
}

impl AttemptOutput {
    fn into_provider_output(self, model: &str) -> ProviderOutput {
        if self.response.insert_text.is_empty() {
            return ProviderOutput::NoSuggestion;
        }
        ProviderOutput::Candidate(CompletionCandidate::new(
            self.response.insert_text,
            self.response.confidence.unwrap_or(0.5),
            format!("pi:{model}"),
        ))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderJsonResponse {
    insert_text: String,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    _reason: Option<String>,
}

fn parse_provider_response(text: &str) -> Result<ProviderJsonResponse, String> {
    let response: ProviderJsonResponse = serde_json::from_str(text)
        .map_err(|error| format!("assistant text was not provider JSON: {error}"))?;
    if let Some(confidence) = response.confidence
        && (!(0.0..=1.0).contains(&confidence) || confidence.is_nan())
    {
        return Err(format!(
            "provider confidence must be between 0 and 1, got {confidence}"
        ));
    }
    Ok(response)
}

async fn read_pipe<R>(mut reader: R) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    if reader.read_to_end(&mut bytes).await.is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn collect_pipe_task(task: tokio::task::JoinHandle<String>, interrupted: bool) -> String {
    if interrupted {
        match tokio::time::timeout(Duration::from_millis(100), task).await {
            Ok(Ok(output)) => output,
            _ => String::new(),
        }
    } else {
        task.await.unwrap_or_default()
    }
}
