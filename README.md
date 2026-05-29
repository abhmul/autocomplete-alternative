# Autocomplete Alternative MVP

Standalone local autocomplete broker with thin VS Code and Obsidian clients. The broker owns protocol validation, context trimming, prompt selection, provider calls, cancellation, privacy policy, and postprocessing; editor clients only collect context, call the broker, render ghost text, and accept/dismiss suggestions.

## Repository map

- `crates/autocomplete-protocol`: Rust protocol types, validation, JSON Schema export, fixtures tests.
- `crates/autocomplete-core`: provider trait, engine, mock provider, postprocessing.
- `crates/autocomplete-addons`: built-in context windowing, privacy policy, prompt template selection, postprocessor registry.
- `crates/autocomplete-provider-pi`: bounded `pi` subprocess provider.
- `crates/autocomplete-server`: local HTTP broker binary `autocomplete-broker`.
- `clients/protocol`: generated JS protocol constants/schemas plus shared broker client/session helper.
- `clients/vscode`: thin VS Code inline completion provider.
- `clients/obsidian`: thin Obsidian CodeMirror ghost-text plugin.
- `examples/fixtures`: protocol request/response examples used by tests and smoke commands.

## Build and test

```sh
cargo fmt --all --check
cargo test
cargo build
npm test --prefix clients
```

Regenerate JS protocol artifacts after Rust protocol changes:

```sh
cargo run -p autocomplete-protocol --bin export_client_artifacts -- clients/protocol/src/generated
```

## Run the broker

Default run uses the mock provider on `127.0.0.1:32145`:

```sh
cargo run -p autocomplete-server --bin autocomplete-broker
```

In another shell:

```sh
curl -s http://127.0.0.1:32145/health
curl -s -H 'content-type: application/json' --data @examples/fixtures/autocomplete-request.v1.json http://127.0.0.1:32145/v1/autocomplete
```

Minimal mock config:

```sh
cat >/tmp/autocomplete-mock.toml <<'TOML'
[provider]
kind = "mock"

[mock]
insert_text = " synthetic completion"
confidence = 0.93
delay_ms = 0
TOML

cargo run -p autocomplete-server --bin autocomplete-broker -- --config /tmp/autocomplete-mock.toml
```

Minimal pi config for the provider/model path that passed the MVP smoke on this host:

```sh
cat >/tmp/autocomplete-pi.toml <<'TOML'
[provider]
kind = "pi"

[pi]
command = "pi"
provider = "openai-codex"
model = "gpt-5.5"
thinking = "minimal"
timeout_ms = 10000
repair_retry = true

[privacy]
remote_context_byte_limit = 6000
excluded_globs = ["**/.env*", "**/secrets/**", "**/prompt-buffer.md"]
TOML

cargo run -p autocomplete-server --bin autocomplete-broker -- --config /tmp/autocomplete-pi.toml
```

Use `pi --list-models` before changing the pi config. The original plan/default `openai/gpt-5.5` path failed on the smoke host because `pi` reported no OpenAI API key; `provider = "openai-codex"` with `model = "gpt-5.5"` succeeded. The broker invokes `pi` with `--mode json`, `--no-tools`, `--no-session`, `--no-context-files`, `--no-extensions`, `--no-skills`, `--no-prompt-templates`, and `--no-themes`.

## Client setup

See `docs/client-setup.md` for VS Code and Obsidian local-source setup. In short: start the broker, point the client setting to `http://127.0.0.1:32145`, and use mock provider first. Both clients call the same `/v1/autocomplete` and `/v1/cancel/<request_id>` protocol and do not know whether the broker uses mock or pi.

## Protocol docs

See `docs/protocol.md` and `crates/autocomplete-protocol/README.md`. Version 1 uses local HTTP JSON:

- `GET /health`
- `POST /v1/autocomplete`
- `POST /v1/cancel/<request_id>`
- `POST /v1/reload`

JSON Schemas are generated into `clients/protocol/src/generated/schemas/`; example payloads are in `examples/fixtures/`.

## Current limitations

- Real pi smoke passed only on synthetic TypeScript and Markdown requests: 4529.306 ms and 6254.653 ms wall-clock, median 5391.979 ms over `n=2`. This is suitable for manual or idle-after-pause MVP behavior, not per-keystroke Copilot-like autocomplete.
- GUI rendering and Tab acceptance are covered by source/host-glue tests, not by real VS Code Extension Development Host or disposable Obsidian vault smoke.
- Obsidian and VS Code packaging is not solved; local source imports expect `clients/protocol` to remain available relative to each client.
- Autocomplete quality is not proven beyond simple synthetic completions. Generic chat-style models may underperform fill-in-the-middle providers.
- `/health` and response `source` currently report `pi:gpt-5.5` and omit the configured `pi.provider` prefix.
- Client UX for broker errors is intentionally minimal: failures normally appear as no suggestion.

More detail and next steps are in `docs/limitations-and-next-steps.md`.

## Next optimization plan

1. Run real editor smoke in VS Code and a disposable Obsidian vault with the mock provider, then with bounded pi calls.
2. Package or bundle clients so shared protocol helpers are present after installation.
3. Reduce provider latency with a persistent pi/RPC worker or a direct provider API, then evaluate FIM-focused providers such as Mistral FIM, Ollama, or llama.cpp infill adapters.
4. Add latency/quality evaluation fixtures and track median/p95 separately for broker overhead and provider time.
5. Improve diagnostics: include configured pi provider in health/source, expose client-visible broker health, and keep privacy/context byte decisions inspectable.
