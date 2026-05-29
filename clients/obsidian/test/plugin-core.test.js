"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const {
  ObsidianAutocompleteController,
  createRequestFromObsidianState,
  obsidianDocumentUri,
} = require("../src/plugin-core.js");

const UUID_1 = "018f160e-7152-7b43-9d9a-6083e0bd3cc8";
const UUID_2 = "018f160e-7152-7b43-9d9a-6083e0bd3cc9";

function waitForTimers() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function fakeState(text, head) {
  const lineStarts = [0];
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === "\n") {
      lineStarts.push(index + 1);
    }
  }
  return {
    doc: {
      toString: () => text,
      lineAt(offset) {
        let lineIndex = 0;
        for (let index = 0; index < lineStarts.length; index += 1) {
          if (lineStarts[index] <= offset) {
            lineIndex = index;
          }
        }
        return { number: lineIndex + 1, from: lineStarts[lineIndex] };
      },
    },
    selection: { main: { head } },
  };
}

test("Obsidian host glue constructs markdown broker requests from note state", () => {
  const state = fakeState("# Heading\nWrite the next sentence", 16);
  const request = createRequestFromObsidianState({
    state,
    file: { path: "Daily Notes/2026-05-28.md" },
    vaultName: "Agent Vault",
    settings: {
      pluginVersion: "0.1.0",
      maxContextChars: 100,
      maxCompletionChars: 180,
      deadlineMs: 2500,
    },
    documentVersion: 3,
    requestId: UUID_1,
  });

  assert.equal(request.client.name, "obsidian");
  assert.equal(request.document.uri, obsidianDocumentUri({ path: "Daily Notes/2026-05-28.md" }, "Agent Vault"));
  assert.equal(request.document.language_id, "markdown");
  assert.equal(request.document.version, 3);
  assert.deepEqual(request.cursor, { line: 1, character: 6, offset: 16 });
  assert.equal(request.context.prefix, "# Heading\nWrite ");
  assert.equal(request.context.suffix, "the next sentence");
  assert.equal(request.options.mode, "inline_tab");
});

test("Obsidian controller cancels stale note requests and accepts fresh ghost text with Tab", async () => {
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
  const ids = [UUID_1, UUID_2];
  const controller = new ObsidianAutocompleteController({
    client,
    settings: { debounceMs: 0, maxContextChars: 100, maxCompletionChars: 180, deadlineMs: 2500 },
    requestIdFactory: () => ids.shift(),
  });

  const first = controller.requestSuggestion({
    state: fakeState("alpha", 5),
    file: { path: "note.md" },
    vaultName: "Vault",
  });
  await waitForTimers();

  const second = controller.requestSuggestion({
    state: fakeState("alpha beta", 10),
    file: { path: "note.md" },
    vaultName: "Vault",
  });
  await waitForTimers();

  assert.deepEqual(cancelCalls, [UUID_1]);
  assert.deepEqual(autocompleteCalls.map((request) => request.request_id), [UUID_1, UUID_2]);
  assert.equal(await first, null);

  resolvers[1]({ status: "ok", request_id: UUID_2, insert_text: " gamma" });
  const suggestion = await second;

  assert.deepEqual(suggestion, {
    requestId: UUID_2,
    from: 10,
    text: " gamma",
    documentVersion: 2,
  });

  const dispatches = [];
  const accepted = controller.accept({
    state: { selection: { main: { head: 10 } } },
    dispatch: (transaction) => dispatches.push(transaction),
  });

  assert.equal(accepted, true);
  assert.deepEqual(dispatches, [
    { changes: { from: 10, insert: " gamma" }, selection: { anchor: 16 } },
  ]);
});
