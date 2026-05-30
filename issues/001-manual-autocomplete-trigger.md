---
title: Manual autocomplete request hotkey
status: open
type: feature-request
tags:
  - ai-generated
  - broker
  - protocol
  - vscode
  - obsidian
---

# Manual autocomplete request hotkey

## Problem

Users need an explicit way to request a completion at the current cursor location without waiting for idle/debounce behavior. This matters for prose/math notes where the cursor may sit at a semantic gap and the user wants a completion on demand, for example:

```md
# Trace of Matrix

## Definition
Let $R$ be a [[Ring]]. Let $A \in \mathrm{M}_{n \times n}(R)$ be the [[Matrix Ring]] over $R$. The [[Trace of Matrix|Trace]] of $A$, denoted $\mathrm{Tr}(A)$ is defined as
$$
    \mathrm{Tr}(A) = \sum_{i=1}^{n} A_{ii},
$$
the sum of the diagonal entries of $A$.

### Properties
1. For $A,B \in [CURSOR]
```

Current behavior is implicit: clients request suggestions after editor changes/selection changes. Obsidian also has no command visible in Hotkeys for “request completion now.”

## Requested behavior

Add a bindable “Autocomplete Alternative: Request completion” command in both VS Code and Obsidian. When invoked, it sends a completion request for the active editor/cursor using trigger `manual`, cancels any stale in-flight request for that editor, and displays the resulting ghost text if available.

## Shared broker/protocol work

- Treat `AutocompleteOptions.trigger = manual` as a first-class path in the broker, not merely a client hint.
- Implement broker-side trigger policy consistently for all clients:
  - `trigger.mode = manual` should reject or ignore idle/document-change requests while allowing manual requests.
  - `trigger.min_prefix_chars` should apply to automatic triggers, with a documented decision on whether manual requests bypass it.
  - Manual requests should receive the same privacy checks, context trimming, provider deadline enforcement, cancellation, and postprocessing as idle requests.
- Add tests covering manual trigger behavior, especially interaction with trigger config and cancellation.
- Update protocol docs to document the semantics of `idle`, `document_change`, and `manual` triggers.

## VS Code integration work

- Register a command such as `autocompleteAlternative.requestCompletion`.
- Contribute it to package metadata so it appears in Keyboard Shortcuts.
- Invoke the existing inline completion provider path with `trigger = manual`, or otherwise reuse the same request construction/session logic.
- Ensure a manual request updates or replaces the active inline ghost text without requiring the user to type another character.

## Obsidian integration work

- Register an Obsidian command such as `Request autocomplete suggestion` so it appears in Obsidian Hotkeys.
- Wire the command to the active Markdown editor/CodeMirror view and call the same controller request path with `trigger = manual`.
- Keep the existing automatic debounce behavior available unless disabled by broker/client settings.
- Consider a default unset hotkey rather than hard-coding a key, to avoid conflicts with existing vault shortcuts.

## Acceptance criteria

- Both VS Code and Obsidian expose a user-visible command that can be keybound.
- Invoking the command at a cursor location sends a request with `trigger = manual`.
- The broker applies documented trigger policy server-side and has tests for manual-vs-automatic behavior.
- The feature works for Markdown/math prose examples like the trace-of-matrix note above.
