Alternative to Copilot autocomplete that supports either GPT subscription or Claude subscription.

- Ignore `tmp/`, `archive/`, or other directories in `~/dev` unless the user explicitly references that path.
- Do not read `prompt-buffer.md`.
- Treat files tagged `human-written` as read-only unless the user explicitly asks to edit them. Agents must not add the `human-written` tag.
- For file-modifying sessions, run the `/checkpoint` skill before ending and at coherent milestones.