"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const {
  createInlineCompletionProvider,
  createRequestFromVsCodeDocument,
} = require("../src/extension-core.js");

const UUID_1 = "018f160e-7152-7b43-9d9a-6083e0bd3cc8";
const UUID_2 = "018f160e-7152-7b43-9d9a-6083e0bd3cc9";

function waitForTimers() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function fakeDocument(text, overrides = {}) {
  const lineStarts = [0];
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === "\n") {
      lineStarts.push(index + 1);
    }
  }
  return {
    uri: { toString: () => overrides.uri || "file:///repo/src/app.ts" },
    languageId: overrides.languageId || "typescript",
    version: overrides.version || 7,
    getText: () => text,
    offsetAt: (position) => lineStarts[position.line] + position.character,
  };
}

function fakeVsCode() {
  return {
    window: { activeTextEditor: null },
    Range: class Range {
      constructor(start, end) {
        this.start = start;
        this.end = end;
      }
    },
    InlineCompletionItem: class InlineCompletionItem {
      constructor(insertText, range) {
        this.insertText = insertText;
        this.range = range;
      }
    },
  };
}

function neverCancelledToken() {
  return {
    isCancellationRequested: false,
    onCancellationRequested() {},
  };
}

test("VS Code host glue constructs broker requests from document cursor metadata", () => {
  const document = fakeDocument("const greeting = \"hello\";\nconsole.log(greeting);", { version: 12 });
  const position = { line: 1, character: 7 };

  const request = createRequestFromVsCodeDocument({
    document,
    position,
    settings: {
      extensionVersion: "0.1.0",
      maxContextChars: 100,
      maxCompletionChars: 180,
      deadlineMs: 2500,
    },
    selectedText: "",
    requestId: UUID_1,
  });

  assert.equal(request.client.name, "vscode");
  assert.equal(request.document.uri, "file:///repo/src/app.ts");
  assert.equal(request.document.language_id, "typescript");
  assert.equal(request.document.version, 12);
  assert.deepEqual(request.cursor, { line: 1, character: 7, offset: 33 });
  assert.equal(request.context.prefix, "const greeting = \"hello\";\nconsole");
  assert.equal(request.context.suffix, ".log(greeting);");
  assert.equal(request.options.mode, "inline_tab");
});

test("VS Code inline provider cancels stale requests before rendering ghost text", async () => {
  const document = fakeDocument("const value = 1;");
  const vscode = fakeVsCode();
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
  const provider = createInlineCompletionProvider({
    vscode,
    client,
    settings: { debounceMs: 0, maxContextChars: 200, maxCompletionChars: 180, deadlineMs: 2500 },
    requestIdFactory: () => ids.shift(),
  });

  const first = provider.provideInlineCompletionItems(
    document,
    { line: 0, character: 6 },
    {},
    neverCancelledToken(),
  );
  await waitForTimers();

  const second = provider.provideInlineCompletionItems(
    document,
    { line: 0, character: 12 },
    {},
    neverCancelledToken(),
  );
  await waitForTimers();

  assert.deepEqual(cancelCalls, [UUID_1]);
  assert.deepEqual(autocompleteCalls.map((request) => request.request_id), [UUID_1, UUID_2]);
  assert.equal(await first, undefined);

  resolvers[1]({ status: "ok", request_id: UUID_2, insert_text: " + 2" });
  const items = await second;

  assert.equal(items.length, 1);
  assert.equal(items[0].insertText, " + 2");
  assert.deepEqual(items[0].range.start, { line: 0, character: 12 });
});
