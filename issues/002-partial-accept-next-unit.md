---
title: Partial autocomplete acceptance
status: open
type: feature-request
tags:
  - ai-generated
  - broker
  - protocol
  - vscode
  - obsidian
---

# Partial autocomplete acceptance

## Problem

Users need a way to accept only part of a visible suggestion, such as the next word, next token, or next semantically useful unit. Current behavior accepts the entire ghost-text suggestion with Tab in Obsidian, and VS Code relies on editor inline-completion behavior without project-defined shared semantics.

This is especially important for mathematical Markdown and LaTeX-heavy notes, where a useful “next unit” may be more than whitespace-delimited text, for example `\mathrm{M}_{n \times n}(R)`, `[[Matrix Ring]]`, `$...$`, or a short phrase.

## Requested behavior

Add bindable commands in both VS Code and Obsidian for partial acceptance, initially “Accept next word/unit.” The command should insert the next server-defined acceptance segment from the active suggestion and leave the remainder visible as ghost text.

## Shared broker/protocol work

Implement the important segmentation logic in the shared broker/protocol layer so VS Code and Obsidian do not diverge.

Recommended approach:

- Extend the autocomplete response with optional acceptance metadata, for example ordered segment boundaries over `insert_text`:
  - byte or character offsets must be explicitly specified and Unicode-safe;
  - include a `kind` when useful, such as `word`, `whitespace`, `math_command`, `wiki_link`, `latex_group`, `punctuation`, or `line`;
  - preserve backward compatibility for clients that ignore the metadata.
- Add a broker-side segmenter in the postprocessing/shared logic path after final `insert_text` is known.
- Make segmentation language-aware enough for Markdown and LaTeX-heavy prose:
  - keep balanced wiki links like `[[Matrix Ring]]` together;
  - keep common LaTeX command/group fragments like `\mathrm{M}` and possibly `\mathrm{M}_{n \times n}(R)` together when safe;
  - avoid producing segments that leave unmatched delimiters, braces, brackets, or dollar signs;
  - fall back to conservative whitespace/punctuation boundaries for unknown languages.
- Add protocol schema updates, generated JS artifacts, fixtures, and Rust/JS tests for segmentation.

Open design question: whether the segmenter belongs in `autocomplete-core` postprocessing, `autocomplete-addons`, or `autocomplete-protocol`. The requirement is that clients consume shared server-produced boundaries rather than reimplementing tokenization independently.

## VS Code integration work

- Register commands such as `autocompleteAlternative.acceptNextUnit` and optionally `autocompleteAlternative.acceptNextWord`.
- Bind command behavior to the active inline suggestion produced by this extension.
- Insert only the next segment and keep the remaining suffix displayed if possible.
- If VS Code’s native inline completion partial-accept APIs are available, adapt them to use server-provided segment boundaries rather than local splitting.

## Obsidian integration work

- Register Obsidian commands such as `Accept next autocomplete unit` so they appear in Hotkeys.
- Keep full-accept behavior available separately.
- Update the CodeMirror ghost-text widget/controller to track accepted segment count or remaining suggestion text.
- On partial accept, insert the next segment at the cursor, advance the cursor, and re-render the remaining ghost text without issuing a new provider request unless the suggestion becomes stale.

## Acceptance criteria

- Both VS Code and Obsidian expose a bindable command for partial acceptance.
- Segment boundaries are produced by shared server/protocol logic and covered by tests.
- Partial accept is Unicode-safe and works for plain prose, Markdown wiki links, and representative LaTeX/math fragments.
- Full accept remains available and unchanged for users who prefer accepting the entire suggestion.
