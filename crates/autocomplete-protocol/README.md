# Autocomplete protocol

This crate defines the Rust source of truth for the MVP local autocomplete broker protocol. The MVP transport is local HTTP with JSON request and response bodies.

## Versioning

The protocol uses both path and payload versioning:

- Endpoints are rooted at `/v1` for version 1.
- Versioned payloads include `protocol_version: 1`.
- Receivers must reject unsupported major versions with `unsupported_protocol_version`.
- Wire field and enum names are `snake_case`; request and response structs deny unknown fields where practical.

## Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Check broker and provider availability. Returns `HealthResponse`. |
| `POST` | `/v1/autocomplete` | Request one inline autocomplete suggestion. Accepts `AutocompleteRequest` and returns `AutocompleteResponse`. |
| `POST` | `/v1/cancel/:request_id` | Best-effort, idempotent cancellation for a pending request. Returns `CancelResponse`. |
| `POST` | `/v1/reload` | Reload broker config/addons during development. Returns `ReloadResponse`. |

## Autocomplete request

Required fields:

- `protocol_version`: integer protocol payload version, currently `1`.
- `request_id`: UUID used to correlate autocomplete and cancellation calls.
- `client`: `{ name, version }` for the calling editor/client.
- `document`: `{ uri, language_id, version }` for the current buffer.
- `cursor`: zero-based `{ line, character, offset }` cursor metadata.
- `context`: explicit editor-supplied `{ prefix, suffix, selected_text }`; the protocol crate does not read files.
- `options`: `{ mode, max_chars, deadline_ms, trigger }`; MVP mode is `inline_tab`.

Semantic validation enforces protocol version, non-empty bounded identifiers, context byte limits, `max_chars > 0`, and `deadline_ms > 0` within configurable limits.

## Autocomplete response

`AutocompleteResponse` is tagged by `status`:

- `ok`: includes `insert_text`, `confidence` in `[0, 1]`, `source`, and optional `metadata`.
- `no_suggestion`: no insertable text is available.
- `cancelled`: request was cancelled before a suggestion was produced.
- `error`: includes `{ code, message }`.

`metadata` contains broker/provider latency fields and whether postprocessing was applied.

## Error codes

Stable wire names:

- `unsupported_protocol_version`
- `invalid_request`
- `context_too_large`
- `max_chars_out_of_range`
- `deadline_out_of_range`
- `provider_timeout`
- `provider_error`
- `provider_malformed_output`
- `cancelled`
- `internal_error`

## JSON Schema and fixtures

`schema::autocomplete_request_schema()` and `schema::autocomplete_response_schema()` generate JSON Schema from the Rust types. `schema::export_schema_files(output_dir)` writes:

- `autocomplete-request.v1.schema.json`
- `autocomplete-response.v1.schema.json`

Example fixtures live in `examples/fixtures/` at the workspace root and are validated by this crate's tests.
