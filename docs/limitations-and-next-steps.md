# Limitations and next steps

## Verified MVP evidence

- Rust workspace tests passed in the preceding smoke task: protocol validation, provider parsing, postprocessing, timeout, cancellation, server endpoints, config loading, reload, and privacy policy.
- Client Node tests passed: shared protocol helper, VS Code request/render host glue, Obsidian request/accept host glue, cancellation, and provider-boundary scanning.
- Broker mock smoke passed over external HTTP with 12 synthetic requests; median 0.921 ms and nearest-rank p95 1.939 ms.
- Broker pi smoke passed over external HTTP with one synthetic TypeScript request and one synthetic Markdown request using `provider = "openai-codex"`, `model = "gpt-5.5"`, and 10 s deadlines.

## Known limitations

- Pi latency is too high for keystroke-level autocomplete on the smoke host: successful real requests took 4529.306 ms and 6254.653 ms, median 5391.979 ms over `n=2` and nearest-rank p95 6254.653 ms. Treat pi-backed MVP autocomplete as manual or idle-after-pause only.
- The plan/default `openai/gpt-5.5` path failed on the smoke host because `pi` reported no OpenAI API key. Docs and configs should prefer explicit `pi --list-models` discovery and should not assume one provider namespace works everywhere.
- Autocomplete quality is not proven. Synthetic examples were plausible, but this does not establish daily-use quality, multiline behavior, or robustness across languages and note styles.
- Real VS Code and Obsidian UI smoke has not been run. Source and host-glue tests verify construction/render paths, but not extension-host loading, ghost-text rendering in a GUI, or Tab acceptance against real editor keymaps/plugins.
- Client packaging is incomplete. Current local-source clients rely on repository-relative access to `clients/protocol`.
- Obsidian module resolution is unproven in a disposable vault. The plugin source parses, but a real plugin load should verify `obsidian` and CodeMirror imports in the target app.
- Broker diagnostics omit configured pi provider namespace in health/source strings; `provider = "openai-codex"` with `model = "gpt-5.5"` still reports `pi:gpt-5.5`.
- Client error UX is minimal. Broker downtime, provider timeouts, malformed provider output, and privacy rejections generally appear as no suggestion.
- The MVP addon system is intentionally static. It trims request-supplied prefix/suffix, selects code/Markdown prompts, enforces privacy globs/byte limits, and runs postprocessors; it does not index workspaces or read neighboring files.
- Local HTTP has no authentication because the broker is bound to loopback for MVP.

## Next-step optimization plan

1. Run real-editor smoke: VS Code Extension Development Host and a disposable Obsidian vault, first with mock provider, then with bounded pi requests and explicit deadlines.
2. Fix client distribution: bundle/copy `clients/protocol` into both clients, add packaging scripts, and document installable artifacts separately from source-layout smoke.
3. Improve diagnostics: include configured pi provider namespace in `/health` and response `source`, add a client-visible health/error indicator, and expose timeout/privacy rejection reasons without leaking sensitive context.
4. Reduce latency: replace per-request pi subprocess startup with a persistent pi worker/RPC path if available, or use direct provider APIs behind the same broker trait.
5. Add FIM-capable providers: evaluate Mistral FIM, Ollama, llama.cpp infill, or other low-latency local/remote adapters while keeping clients unchanged.
6. Add quality and latency harnesses: fixed code/Markdown fixture suites, accepted/rejected output checks, median/p95 over meaningful sample sizes, and separate broker overhead from provider time.
7. Tune triggering: manual trigger first, then idle-after-pause with adaptive debounce, cancellation on document/cursor changes, and conservative max context until latency and quality are acceptable.
8. Expand privacy policy before richer context: enforce workspace allowlists, sensitive glob defaults, remote byte limits, and explicit local/remote provider state before adding open-file or workspace-symbol context.
