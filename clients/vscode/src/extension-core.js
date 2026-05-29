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
} = require("../../protocol/src/client.js");

const CLIENT_NAME = "vscode";
const DEFAULT_EXTENSION_VERSION = "0.1.0";

function readVsCodeSettings(vscode, extensionVersion = DEFAULT_EXTENSION_VERSION) {
  const configuration = vscode.workspace.getConfiguration("autocompleteAlternative");
  return defaultClientSettings({
    brokerUrl: configuration.get("brokerUrl", DEFAULT_BROKER_URL),
    debounceMs: configuration.get("debounceMs", DEFAULT_DEBOUNCE_MS),
    maxContextChars: configuration.get("maxContextChars", DEFAULT_MAX_CONTEXT_CHARS),
    maxCompletionChars: configuration.get("maxCompletionChars", DEFAULT_MAX_COMPLETION_CHARS),
    deadlineMs: configuration.get("deadlineMs", DEFAULT_DEADLINE_MS),
    extensionVersion,
  });
}

function selectedTextForDocument(vscode, document) {
  const editor = vscode.window && vscode.window.activeTextEditor;
  if (!editor || editor.document !== document || !editor.selection || editor.selection.isEmpty) {
    return "";
  }
  return document.getText(editor.selection);
}

function createRequestFromVsCodeDocument({
  document,
  position,
  settings,
  selectedText = "",
  requestId,
  trigger = TRIGGER_IDLE,
}) {
  const text = document.getText();
  const offset = document.offsetAt(position);
  const context = contextAroundText(text, offset, settings.maxContextChars);
  return createAutocompleteRequest({
    requestId,
    clientName: CLIENT_NAME,
    clientVersion: settings.extensionVersion || DEFAULT_EXTENSION_VERSION,
    documentUri: document.uri.toString(),
    languageId: document.languageId,
    documentVersion: document.version,
    cursor: {
      line: position.line,
      character: position.character,
      offset,
    },
    context: {
      ...context,
      selected_text: selectedText,
    },
    options: {
      max_chars: settings.maxCompletionChars,
      deadline_ms: settings.deadlineMs,
      trigger,
    },
  });
}

function abortSignalFromVsCodeToken(token) {
  const controller = new AbortController();
  if (!token) {
    return controller.signal;
  }
  if (token.isCancellationRequested) {
    controller.abort("vscode_token_cancelled");
    return controller.signal;
  }
  token.onCancellationRequested(() => controller.abort("vscode_token_cancelled"));
  return controller.signal;
}

function createInlineCompletionProvider({ vscode, settings, client, session, requestIdFactory }) {
  const resolvedSettings = defaultClientSettings(settings || {});
  const resolvedClient = client || new BrokerClient({ brokerUrl: resolvedSettings.brokerUrl });
  const autocompleteSession =
    session || new BrokerAutocompleteSession({ client: resolvedClient, debounceMs: resolvedSettings.debounceMs });

  return {
    async provideInlineCompletionItems(document, position, context, token) {
      const request = createRequestFromVsCodeDocument({
        document,
        position,
        settings: resolvedSettings,
        selectedText: selectedTextForDocument(vscode, document),
        requestId: requestIdFactory ? requestIdFactory() : undefined,
        trigger: triggerFromVsCodeContext(context),
      });

      let response;
      try {
        response = await autocompleteSession.complete(request, {
          signal: abortSignalFromVsCodeToken(token),
        });
      } catch {
        return undefined;
      }
      const insertText = insertTextFromResponse(response, request.request_id);
      if (!insertText) {
        return undefined;
      }

      const range = new vscode.Range(position, position);
      return [new vscode.InlineCompletionItem(insertText, range)];
    },

    dispose() {
      autocompleteSession.cancelCurrent("provider_disposed");
    },
  };
}

function triggerFromVsCodeContext(context) {
  if (context && context.triggerKind && String(context.triggerKind).toLowerCase().includes("invoke")) {
    return "manual";
  }
  return TRIGGER_IDLE;
}

module.exports = {
  CLIENT_NAME,
  readVsCodeSettings,
  selectedTextForDocument,
  createRequestFromVsCodeDocument,
  abortSignalFromVsCodeToken,
  createInlineCompletionProvider,
};
