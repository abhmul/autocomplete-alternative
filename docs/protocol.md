# Protocol v1

The broker protocol is local HTTP with JSON bodies. The Rust source of truth is `crates/autocomplete-protocol`; generated JS constants and JSON Schemas live under `clients/protocol/src/generated/`.

## Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Broker liveness plus provider name/status. |
| `POST` | `/v1/autocomplete` | Request one inline autocomplete suggestion. |
| `POST` | `/v1/cancel/<request_id>` | Best-effort cancellation for a pending request. |
| `POST` | `/v1/reload` | Reload config/addons when the broker was started with `--config`. |

## Request contract

`AutocompleteRequest` fields:

- `protocol_version`: currently `1`.
- `request_id`: UUID used for correlation and cancellation.
- `client`: `{ name, version }`; current clients use `vscode` and `obsidian`.
- `document`: `{ uri, language_id, version }`.
- `cursor`: zero-based `{ line, character, offset }`.
- `context`: explicit editor-supplied `{ prefix, suffix, selected_text }`; protocol and broker context providers do not read files for MVP.
- `options`: `{ mode, max_chars, deadline_ms, trigger }`; current mode is `inline_tab`.

Example:

```sh
curl -s -H 'content-type: application/json' --data @examples/fixtures/autocomplete-request.v1.json http://127.0.0.1:32145/v1/autocomplete
```

## Response contract

`AutocompleteResponse` is tagged by `status`:

- `ok`: includes `insert_text`, `confidence`, `source`, and optional latency/postprocessing metadata.
- `no_suggestion`: the broker/provider had no insertable text.
- `cancelled`: the request was cancelled before completion.
- `error`: includes `{ code, message }`.

Stable error codes are `unsupported_protocol_version`, `invalid_request`, `context_too_large`, `max_chars_out_of_range`, `deadline_out_of_range`, `provider_timeout`, `provider_error`, `provider_malformed_output`, `cancelled`, and `internal_error`.

## Generated artifacts and fixtures

- Rust docs and schema generation: `crates/autocomplete-protocol/README.md`.
- JSON Schemas: `clients/protocol/src/generated/schemas/autocomplete-request.v1.schema.json` and `clients/protocol/src/generated/schemas/autocomplete-response.v1.schema.json`.
- Fixture payloads: `examples/fixtures/autocomplete-request.v1.json`, `examples/fixtures/autocomplete-response-ok.v1.json`, and `examples/fixtures/autocomplete-response-error.v1.json`.
- Regeneration command: `cargo run -p autocomplete-protocol --bin export_client_artifacts -- clients/protocol/src/generated`.

## Invariants

- Version is carried in both `/v1` paths and `protocol_version` payload fields.
- Clients never select providers, prompts, or postprocessors.
- Provider output is postprocessed before any `ok` response reaches clients.
- Cancellation is best-effort; stale clients should ignore late responses by `request_id`.
- Privacy exclusions and remote context byte limits are enforced in the broker before remote provider calls.
