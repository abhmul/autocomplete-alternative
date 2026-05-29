# Client setup

Start with the mock provider. Once ghost text and cancellation work against mock, switch the broker config to pi.

## Shared prerequisites

```sh
cargo run -p autocomplete-server --bin autocomplete-broker
curl -s http://127.0.0.1:32145/health
```

Expected mock health includes `"status":"ok"` and provider name `"mock"`. Both clients default to `http://127.0.0.1:32145`.

## VS Code local-source client

Run VS Code with the extension development path pointing at the repository client folder:

```sh
code --extensionDevelopmentPath="$PWD/clients/vscode" "$PWD"
```

Useful workspace settings:

```json
{
  "autocompleteAlternative.brokerUrl": "http://127.0.0.1:32145",
  "autocompleteAlternative.debounceMs": 500,
  "autocompleteAlternative.maxContextChars": 4700,
  "autocompleteAlternative.maxCompletionChars": 180,
  "autocompleteAlternative.deadlineMs": 2500
}
```

Open a source or Markdown file, type near an incomplete expression or sentence, and wait for the debounce window or manually trigger inline suggestions. VS Code accepts inline suggestions with its normal inline-completion acceptance flow, commonly Tab when no higher-priority binding intercepts it.

Notes:

- The VS Code client imports `clients/protocol` by repository-relative path. Use the repository layout for local testing; packaged distribution needs a bundle/copy step.
- Settings are read when the extension/provider is created. Reload the extension host after changing settings if behavior does not update.
- Broker/network errors are swallowed as no suggestion in the current MVP client.

## Obsidian local artifact client

Use a disposable vault for MVP smoke. The Obsidian plugin artifact lives at `clients/obsidian` and has the root files Obsidian expects: `manifest.json`, `main.js`, and `styles.css`. `main.js` is a generated bundle of the Obsidian client plus shared protocol helper; it intentionally leaves only Obsidian/CodeMirror runtime packages external and contains no provider logic.

After changing `clients/obsidian/src/*` or `clients/protocol/src/*`, rebuild the artifact:

```sh
npm run --prefix clients build:obsidian
```

Symlink the plugin artifact into a disposable vault:

```sh
VAULT=/path/to/disposable-vault
mkdir -p "$VAULT/.obsidian/plugins"
ln -s "$PWD/clients/obsidian" "$VAULT/.obsidian/plugins/autocomplete-alternative"
```

If symlinks are not acceptable, copy only the artifact root files:

```sh
VAULT=/path/to/disposable-vault
PLUGIN="$VAULT/.obsidian/plugins/autocomplete-alternative"
mkdir -p "$PLUGIN"
cp clients/obsidian/manifest.json clients/obsidian/main.js clients/obsidian/styles.css "$PLUGIN/"
```

Open the vault, enable Community plugins, enable `Autocomplete Alternative`, and set the Broker URL to `http://127.0.0.1:32145`. Type in a Markdown note and use Tab to accept a visible ghost-text suggestion.

Notes:

- Do not symlink `clients/protocol` into the vault; the protocol helper is already bundled into `clients/obsidian/main.js`.
- Existing editor views may need to be reopened after settings changes or after rebuilding the artifact.

## Provider switching

Clients do not change when switching providers. Restart or reload the broker with a different config:

```toml
[provider]
kind = "mock"
```

or:

```toml
[provider]
kind = "pi"

[pi]
command = "pi"
provider = "openai-codex"
model = "gpt-5.5"
thinking = "minimal"
timeout_ms = 10000
repair_retry = true
```

For pi smoke, raise the client `deadlineMs` enough to cover expected provider latency. The MVP smoke observed roughly 4.5 s and 6.3 s for two synthetic pi-backed requests, so the default 2500 ms client deadline is too low for that host.

## Validation commands

```sh
npm run --prefix clients build:obsidian
node clients/obsidian/scripts/build-artifact.js --check
npm test --prefix clients
node --check clients/vscode/src/extension.js
node --check clients/obsidian/main.js
```

These checks verify request construction, debounce/cancellation, provider-boundary tests, Obsidian artifact freshness/layout, and syntax. They do not replace real editor-host smoke.
