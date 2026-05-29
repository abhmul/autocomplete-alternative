"use strict";

const {
  DEFAULT_BROKER_URL,
  DEFAULT_DEBOUNCE_MS,
  DEFAULT_MAX_CONTEXT_CHARS,
  DEFAULT_MAX_COMPLETION_CHARS,
  DEFAULT_DEADLINE_MS,
  TRIGGER_IDLE,
  BrokerAutocompleteSession,
  BrokerClient,
  contextAroundText,
  createAutocompleteRequest,
  defaultClientSettings,
  insertTextFromResponse,
  lineCharacterFromOffset,
} = require("../../protocol/src/client.js");

const CLIENT_NAME = "obsidian";
const DEFAULT_PLUGIN_VERSION = "0.1.0";

const DEFAULT_OBSIDIAN_SETTINGS = defaultClientSettings({
  brokerUrl: DEFAULT_BROKER_URL,
  debounceMs: DEFAULT_DEBOUNCE_MS,
  maxContextChars: DEFAULT_MAX_CONTEXT_CHARS,
  maxCompletionChars: DEFAULT_MAX_COMPLETION_CHARS,
  deadlineMs: DEFAULT_DEADLINE_MS,
  pluginVersion: DEFAULT_PLUGIN_VERSION,
});

function normalizeObsidianSettings(settings = {}) {
  const overrides = settings && typeof settings === "object" ? settings : {};
  return defaultClientSettings({ ...DEFAULT_OBSIDIAN_SETTINGS, ...overrides });
}

function obsidianDocumentUri(file, vaultName = "vault") {
  const filePath = file && file.path ? file.path : "untitled.md";
  const encodedVault = encodeURIComponent(vaultName || "vault");
  const encodedPath = filePath.split("/").map(encodeURIComponent).join("/");
  return `obsidian://vault/${encodedVault}/${encodedPath}`;
}

function textFromState(state) {
  if (!state || !state.doc) {
    return "";
  }
  return typeof state.doc.toString === "function" ? state.doc.toString() : String(state.doc);
}

function cursorFromState(state) {
  const text = textFromState(state);
  const offset = state && state.selection && state.selection.main
    ? state.selection.main.head
    : 0;
  if (state && state.doc && typeof state.doc.lineAt === "function") {
    const line = state.doc.lineAt(offset);
    return {
      line: Math.max(0, line.number - 1),
      character: offset - line.from,
      offset,
    };
  }
  return lineCharacterFromOffset(text, offset);
}

function createRequestFromObsidianState({
  state,
  file,
  vaultName,
  settings,
  documentVersion = 0,
  selectedText = "",
  requestId,
  trigger = TRIGGER_IDLE,
}) {
  const resolvedSettings = normalizeObsidianSettings(settings);
  const text = textFromState(state);
  const cursor = cursorFromState(state);
  const context = contextAroundText(text, cursor.offset, resolvedSettings.maxContextChars);
  return createAutocompleteRequest({
    requestId,
    clientName: CLIENT_NAME,
    clientVersion: resolvedSettings.pluginVersion || DEFAULT_PLUGIN_VERSION,
    documentUri: obsidianDocumentUri(file, vaultName),
    languageId: "markdown",
    documentVersion,
    cursor,
    context: {
      ...context,
      selected_text: selectedText,
    },
    options: {
      max_chars: resolvedSettings.maxCompletionChars,
      deadline_ms: resolvedSettings.deadlineMs,
      trigger,
    },
  });
}

class ObsidianAutocompleteController {
  constructor({ settings, client, session, requestIdFactory } = {}) {
    this.settings = normalizeObsidianSettings(settings);
    this.client = client || new BrokerClient({ brokerUrl: this.settings.brokerUrl });
    this.session = session || new BrokerAutocompleteSession({
      client: this.client,
      debounceMs: this.settings.debounceMs,
    });
    this.requestIdFactory = requestIdFactory;
    this.documentVersion = 0;
    this.suggestion = null;
  }

  cancelStale(reason = "obsidian_state_changed") {
    this.session.cancelCurrent(reason);
    this.suggestion = null;
  }

  async requestSuggestion({ state, file, vaultName, trigger = TRIGGER_IDLE }) {
    const documentVersion = ++this.documentVersion;
    const request = createRequestFromObsidianState({
      state,
      file,
      vaultName,
      settings: this.settings,
      documentVersion,
      requestId: this.requestIdFactory ? this.requestIdFactory() : undefined,
      trigger,
    });
    const cursorOffset = request.cursor.offset;
    let response;
    try {
      response = await this.session.complete(request);
    } catch {
      return null;
    }
    const insertText = insertTextFromResponse(response, request.request_id);
    if (!insertText || documentVersion !== this.documentVersion) {
      return null;
    }
    this.suggestion = {
      requestId: request.request_id,
      from: cursorOffset,
      text: insertText,
      documentVersion,
    };
    return this.suggestion;
  }

  accept(view) {
    if (!this.suggestion || !view || !view.state || !view.state.selection) {
      return false;
    }
    const head = view.state.selection.main.head;
    if (head !== this.suggestion.from) {
      this.cancelStale("cursor_moved_before_accept");
      return false;
    }
    const insertText = this.suggestion.text;
    view.dispatch({
      changes: { from: head, insert: insertText },
      selection: { anchor: head + insertText.length },
    });
    this.suggestion = null;
    return true;
  }
}

module.exports = {
  CLIENT_NAME,
  DEFAULT_OBSIDIAN_SETTINGS,
  normalizeObsidianSettings,
  obsidianDocumentUri,
  textFromState,
  cursorFromState,
  createRequestFromObsidianState,
  ObsidianAutocompleteController,
};
