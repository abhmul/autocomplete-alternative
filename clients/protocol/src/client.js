"use strict";

const { randomUUID } = require("node:crypto");
const protocol = require("./generated/protocol-v1.js");

const DEFAULT_BROKER_URL = "http://127.0.0.1:32145";
const DEFAULT_DEBOUNCE_MS = 500;
const DEFAULT_MAX_CONTEXT_CHARS = 4_700;
const DEFAULT_MAX_COMPLETION_CHARS = 180;
const DEFAULT_DEADLINE_MS = 2_500;

function defaultClientSettings(overrides = {}) {
  return {
    brokerUrl: DEFAULT_BROKER_URL,
    debounceMs: DEFAULT_DEBOUNCE_MS,
    maxContextChars: DEFAULT_MAX_CONTEXT_CHARS,
    maxCompletionChars: DEFAULT_MAX_COMPLETION_CHARS,
    deadlineMs: DEFAULT_DEADLINE_MS,
    ...withoutUndefined(overrides),
  };
}

function withoutUndefined(object) {
  return Object.fromEntries(
    Object.entries(object).filter(([, value]) => value !== undefined),
  );
}

function normalizeBrokerUrl(brokerUrl = DEFAULT_BROKER_URL) {
  return String(brokerUrl || DEFAULT_BROKER_URL).replace(/\/+$/, "");
}

function protocolUrl(brokerUrl, path) {
  return `${normalizeBrokerUrl(brokerUrl)}${path}`;
}

function clampInteger(value, fallback, { min = 0, max = Number.MAX_SAFE_INTEGER } = {}) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, Math.trunc(numeric)));
}

function contextAroundText(text, offset, maxContextChars) {
  const source = String(text ?? "");
  const safeOffset = clampInteger(offset, 0, { min: 0, max: source.length });
  const maxChars = clampInteger(maxContextChars, DEFAULT_MAX_CONTEXT_CHARS, { min: 0 });
  const prefixBudget = Math.ceil(maxChars * 0.75);
  const suffixBudget = Math.max(0, maxChars - prefixBudget);
  return {
    prefix: source.slice(Math.max(0, safeOffset - prefixBudget), safeOffset),
    suffix: source.slice(safeOffset, Math.min(source.length, safeOffset + suffixBudget)),
  };
}

function lineCharacterFromOffset(text, offset) {
  const source = String(text ?? "");
  const safeOffset = clampInteger(offset, 0, { min: 0, max: source.length });
  let line = 0;
  let lineStart = 0;
  for (let index = 0; index < safeOffset; index += 1) {
    if (source.charCodeAt(index) === 10) {
      line += 1;
      lineStart = index + 1;
    }
  }
  return { line, character: safeOffset - lineStart, offset: safeOffset };
}

function createAutocompleteRequest({
  requestId = randomUUID(),
  clientName,
  clientVersion = "0.1.0",
  documentUri,
  languageId,
  documentVersion = 0,
  cursor,
  context,
  options = {},
}) {
  if (!clientName) {
    throw new Error("clientName is required");
  }
  if (!documentUri) {
    throw new Error("documentUri is required");
  }
  if (!languageId) {
    throw new Error("languageId is required");
  }
  if (!cursor) {
    throw new Error("cursor is required");
  }
  if (!context) {
    throw new Error("context is required");
  }

  return {
    protocol_version: protocol.PROTOCOL_VERSION,
    request_id: requestId,
    client: {
      name: String(clientName),
      version: String(clientVersion || "0.1.0"),
    },
    document: {
      uri: String(documentUri),
      language_id: String(languageId),
      version: clampInteger(documentVersion, 0, { min: 0 }),
    },
    cursor: {
      line: clampInteger(cursor.line, 0, { min: 0 }),
      character: clampInteger(cursor.character, 0, { min: 0 }),
      offset: clampInteger(cursor.offset, 0, { min: 0 }),
    },
    context: {
      prefix: String(context.prefix ?? ""),
      suffix: String(context.suffix ?? ""),
      selected_text: String(context.selected_text ?? ""),
    },
    options: {
      mode: protocol.AUTOCOMPLETE_MODE_INLINE_TAB,
      max_chars: clampInteger(options.max_chars, DEFAULT_MAX_COMPLETION_CHARS, { min: 1 }),
      deadline_ms: clampInteger(options.deadline_ms, DEFAULT_DEADLINE_MS, { min: 1 }),
      trigger: options.trigger || protocol.TRIGGER_IDLE,
    },
  };
}

function insertTextFromResponse(response, requestId) {
  if (!response || response.status !== protocol.RESPONSE_OK) {
    return null;
  }
  if (requestId && response.request_id !== requestId) {
    return null;
  }
  const insertText = response.insert_text;
  return typeof insertText === "string" && insertText.length > 0 ? insertText : null;
}

class BrokerClient {
  constructor({ brokerUrl = DEFAULT_BROKER_URL, fetchImpl } = {}) {
    const resolvedFetch = fetchImpl === undefined ? globalThis.fetch : fetchImpl;
    if (typeof resolvedFetch !== "function") {
      throw new Error("fetch is not available; pass fetchImpl when constructing BrokerClient");
    }
    this.brokerUrl = normalizeBrokerUrl(brokerUrl);
    this.fetchImpl = fetchImpl === undefined ? resolvedFetch.bind(globalThis) : fetchImpl;
  }

  async autocomplete(request, { signal } = {}) {
    const response = await this.fetchImpl(protocolUrl(this.brokerUrl, protocol.AUTOCOMPLETE_PATH), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(request),
      signal,
    });
    return response.json();
  }

  async cancel(requestId, { signal } = {}) {
    const response = await this.fetchImpl(
      protocolUrl(this.brokerUrl, `${protocol.CANCEL_PATH_PREFIX}${encodeURIComponent(requestId)}`),
      { method: "POST", signal },
    );
    return response.json();
  }
}

class BrokerAutocompleteSession {
  constructor({ client, debounceMs = DEFAULT_DEBOUNCE_MS } = {}) {
    if (!client) {
      throw new Error("BrokerAutocompleteSession requires a client");
    }
    this.client = client;
    this.debounceMs = clampInteger(debounceMs, DEFAULT_DEBOUNCE_MS, { min: 0 });
    this.sequence = 0;
    this.current = null;
  }

  complete(request, { signal } = {}) {
    this.cancelCurrent("stale_request_replaced");

    const sequence = ++this.sequence;
    const controller = new AbortController();
    let externalAbortCleanup = () => {};

    const promise = new Promise((resolve, reject) => {
      const pending = {
        sequence,
        request,
        requestId: request.request_id,
        controller,
        timer: null,
        settled: false,
        resolve: (value) => {
          if (pending.settled) {
            return;
          }
          pending.settled = true;
          externalAbortCleanup();
          resolve(value);
        },
        reject: (error) => {
          if (pending.settled) {
            return;
          }
          pending.settled = true;
          externalAbortCleanup();
          reject(error);
        },
      };

      if (signal) {
        if (signal.aborted) {
          pending.resolve(null);
          return;
        }
        const onAbort = () => this.cancelPending(pending, "host_cancelled");
        signal.addEventListener("abort", onAbort, { once: true });
        externalAbortCleanup = () => signal.removeEventListener("abort", onAbort);
      }

      this.current = pending;
      pending.timer = setTimeout(() => this.sendPending(pending), this.debounceMs);
    });

    return promise;
  }

  cancelCurrent(reason = "cancelled") {
    if (!this.current) {
      return;
    }
    this.cancelPending(this.current, reason);
  }

  cancelPending(pending, reason = "cancelled") {
    if (pending.timer) {
      clearTimeout(pending.timer);
      pending.timer = null;
    }
    if (!pending.controller.signal.aborted) {
      pending.controller.abort(reason);
    }
    if (this.current === pending) {
      this.current = null;
    }
    this.client.cancel(pending.requestId).catch(() => {});
    pending.resolve(null);
  }

  async sendPending(pending) {
    pending.timer = null;
    if (this.current !== pending || pending.controller.signal.aborted) {
      pending.resolve(null);
      return;
    }

    try {
      const response = await this.client.autocomplete(pending.request, {
        signal: pending.controller.signal,
      });
      if (this.current !== pending || pending.controller.signal.aborted) {
        pending.resolve(null);
        return;
      }
      this.current = null;
      pending.resolve(response);
    } catch (error) {
      if (pending.controller.signal.aborted || this.current !== pending || isAbortError(error)) {
        pending.resolve(null);
        return;
      }
      this.current = null;
      pending.reject(error);
    }
  }
}

function isAbortError(error) {
  return error && (error.name === "AbortError" || error.code === "ABORT_ERR");
}

module.exports = {
  ...protocol,
  DEFAULT_BROKER_URL,
  DEFAULT_DEBOUNCE_MS,
  DEFAULT_MAX_CONTEXT_CHARS,
  DEFAULT_MAX_COMPLETION_CHARS,
  DEFAULT_DEADLINE_MS,
  defaultClientSettings,
  normalizeBrokerUrl,
  protocolUrl,
  contextAroundText,
  lineCharacterFromOffset,
  createAutocompleteRequest,
  insertTextFromResponse,
  BrokerClient,
  BrokerAutocompleteSession,
};
