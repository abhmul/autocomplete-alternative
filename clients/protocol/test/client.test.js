"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const {
  AUTOCOMPLETE_PATH,
  BrokerAutocompleteSession,
  BrokerClient,
  PROTOCOL_VERSION,
  contextAroundText,
  createAutocompleteRequest,
  insertTextFromResponse,
  protocolUrl,
} = require("../src/client.js");

const UUID_1 = "018f160e-7152-7b43-9d9a-6083e0bd3cc8";
const UUID_2 = "018f160e-7152-7b43-9d9a-6083e0bd3cc9";

function waitForTimers() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

test("request construction uses the generated v1 protocol shape", () => {
  const request = createAutocompleteRequest({
    requestId: UUID_1,
    clientName: "test-client",
    clientVersion: "9.8.7",
    documentUri: "file:///repo/src/app.ts",
    languageId: "typescript",
    documentVersion: 12,
    cursor: { line: 2, character: 4, offset: 10 },
    context: { prefix: "const ", suffix: " = 1;", selected_text: "" },
    options: { max_chars: 180, deadline_ms: 2500, trigger: "idle" },
  });

  assert.equal(request.protocol_version, PROTOCOL_VERSION);
  assert.deepEqual(request, {
    protocol_version: 1,
    request_id: UUID_1,
    client: { name: "test-client", version: "9.8.7" },
    document: { uri: "file:///repo/src/app.ts", language_id: "typescript", version: 12 },
    cursor: { line: 2, character: 4, offset: 10 },
    context: { prefix: "const ", suffix: " = 1;", selected_text: "" },
    options: { mode: "inline_tab", max_chars: 180, deadline_ms: 2500, trigger: "idle" },
  });
});

test("contextAroundText clips a bounded window around the cursor", () => {
  assert.deepEqual(contextAroundText("abcdefghijklmnopqrstuvwxyz", 13, 8), {
    prefix: "hijklm",
    suffix: "no",
  });
});

test("BrokerClient calls the shared /v1/autocomplete endpoint", async () => {
  const calls = [];
  const client = new BrokerClient({
    brokerUrl: "http://127.0.0.1:32145/",
    fetchImpl: async (url, options) => {
      calls.push({ url, options });
      return {
        async json() {
          return { status: "no_suggestion", request_id: UUID_1 };
        },
      };
    },
  });

  await client.autocomplete({ request_id: UUID_1 });

  assert.equal(calls[0].url, protocolUrl("http://127.0.0.1:32145", AUTOCOMPLETE_PATH));
  assert.equal(calls[0].options.method, "POST");
  assert.equal(calls[0].options.headers["content-type"], "application/json");
});

test("BrokerClient default fetch uses the global receiver browser hosts require", async (t) => {
  const originalFetch = globalThis.fetch;
  t.after(() => {
    globalThis.fetch = originalFetch;
  });
  const calls = [];
  globalThis.fetch = async function fetchRequiringGlobalReceiver(url, options) {
    if (this !== globalThis) {
      throw new TypeError("Illegal invocation");
    }
    calls.push({ url, options });
    return {
      async json() {
        return { status: "no_suggestion", request_id: UUID_1 };
      },
    };
  };

  const client = new BrokerClient({ brokerUrl: "http://127.0.0.1:32145/" });
  const response = await client.autocomplete({ request_id: UUID_1 });

  assert.equal(response.status, "no_suggestion");
  assert.equal(calls[0].url, protocolUrl("http://127.0.0.1:32145", AUTOCOMPLETE_PATH));
});

test("BrokerAutocompleteSession cancels stale in-flight requests and ignores their late results", async () => {
  const autocompleteCalls = [];
  const cancelCalls = [];
  const resolvers = [];
  const client = {
    autocomplete(request) {
      autocompleteCalls.push(request);
      return new Promise((resolve) => resolvers.push(resolve));
    },
    cancel(requestId) {
      cancelCalls.push(requestId);
      return Promise.resolve({ status: "cancelled", request_id: requestId });
    },
  };
  const session = new BrokerAutocompleteSession({ client, debounceMs: 0 });
  const first = session.complete({ request_id: UUID_1 });
  await waitForTimers();

  const second = session.complete({ request_id: UUID_2 });
  await waitForTimers();

  assert.deepEqual(cancelCalls, [UUID_1]);
  assert.deepEqual(autocompleteCalls.map((request) => request.request_id), [UUID_1, UUID_2]);
  assert.equal(await first, null);

  resolvers[0]({ status: "ok", request_id: UUID_1, insert_text: "stale" });
  resolvers[1]({ status: "ok", request_id: UUID_2, insert_text: "fresh" });
  const response = await second;

  assert.equal(insertTextFromResponse(response, UUID_2), "fresh");
});
