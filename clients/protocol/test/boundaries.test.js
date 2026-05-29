"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

function listJavaScriptAndJsonFiles(dir) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return entry.name === "node_modules" ? [] : listJavaScriptAndJsonFiles(fullPath);
    }
    return /\.(js|json)$/.test(entry.name) ? [fullPath] : [];
  });
}

test("editor clients do not import provider internals", () => {
  const clientsRoot = path.resolve(__dirname, "../..");
  const forbidden = [
    "autocomplete-" + "provider" + "-" + "pi",
    "provider" + "-" + "pi",
    "Pi" + "Provider",
  ];

  const offenders = [];
  for (const file of listJavaScriptAndJsonFiles(clientsRoot)) {
    const text = fs.readFileSync(file, "utf8");
    for (const token of forbidden) {
      if (text.includes(token)) {
        offenders.push(`${path.relative(clientsRoot, file)} contains ${token}`);
      }
    }
  }

  assert.deepEqual(offenders, []);
});
