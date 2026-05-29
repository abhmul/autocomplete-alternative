"use strict";

const assert = require("node:assert/strict");
const childProcess = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

function stubObsidianRuntimeRequire(requested) {
  return (specifier) => {
    requested.push(specifier);
    switch (specifier) {
      case "node:crypto":
        return { randomUUID: () => "018f160e-7152-7b43-9d9a-6083e0bd3cc8" };
      case "obsidian":
        return {
          Plugin: class Plugin {
            async loadData() { return {}; }
            async saveData() {}
            registerEditorExtension() {}
            addSettingTab() {}
          },
          PluginSettingTab: class PluginSettingTab {
            constructor(app, plugin) {
              this.app = app;
              this.plugin = plugin;
              this.containerEl = { empty() {} };
            }
          },
          Setting: class Setting {
            constructor(containerEl) { this.containerEl = containerEl; }
            setName() { return this; }
            setDesc() { return this; }
            addText(callback) {
              const text = {
                setValue() { return text; },
                onChange() { return text; },
              };
              callback(text);
              return this;
            }
          },
        };
      case "@codemirror/view":
        return {
          EditorView: { theme: () => ({}) },
          Decoration: {
            none: { type: "none" },
            set: (decorations) => ({ decorations }),
            widget: ({ widget, side }) => ({ widget, side, range: (from) => ({ from }) }),
          },
          ViewPlugin: { fromClass: (klass, spec) => ({ klass, spec }) },
          WidgetType: class WidgetType {},
          keymap: { of: (bindings) => ({ bindings }) },
        };
      case "@codemirror/state":
        return { Prec: { highest: (extension) => extension } };
      default:
        throw new Error(`unexpected host require: ${specifier}`);
    }
  };
}

function loadArtifactWithStubbedObsidianRuntime(mainPath) {
  const code = fs.readFileSync(mainPath, "utf8");
  const requested = [];
  const module = { exports: {} };
  const sandbox = {
    AbortController,
    clearTimeout,
    console,
    fetch: async () => ({ json: async () => ({ status: "no_suggestion" }) }),
    module,
    exports: module.exports,
    require: stubObsidianRuntimeRequire(requested),
    setTimeout,
  };
  sandbox.globalThis = sandbox;
  vm.runInNewContext(code, sandbox, { filename: mainPath });
  return { exported: module.exports, requested };
}

test("Obsidian plugin artifact exposes loadable root files for a disposable vault", (t) => {
  const pluginRoot = path.resolve(__dirname, "..");
  const vaultRoot = fs.mkdtempSync(path.join(os.tmpdir(), "autocomplete-obsidian-artifact-"));
  t.after(() => fs.rmSync(vaultRoot, { recursive: true, force: true }));

  const pluginsDir = path.join(vaultRoot, ".obsidian", "plugins");
  const pluginDir = path.join(pluginsDir, "autocomplete-alternative");
  fs.mkdirSync(pluginsDir, { recursive: true });
  fs.symlinkSync(pluginRoot, pluginDir, "dir");

  for (const fileName of ["manifest.json", "main.js", "styles.css"]) {
    assert.ok(fs.statSync(path.join(pluginDir, fileName)).isFile(), `missing root ${fileName}`);
  }

  const mainPath = path.join(pluginDir, "main.js");
  const freshness = childProcess.spawnSync(
    process.execPath,
    [path.join(pluginRoot, "scripts", "build-artifact.js"), "--check"],
    { encoding: "utf8" },
  );
  assert.equal(freshness.status, 0, freshness.stderr || freshness.stdout);

  const syntax = childProcess.spawnSync(process.execPath, ["--check", mainPath], { encoding: "utf8" });
  assert.equal(syntax.status, 0, syntax.stderr || syntax.stdout);

  const { exported, requested } = loadArtifactWithStubbedObsidianRuntime(mainPath);
  assert.equal(typeof exported, "function");
  assert.equal(typeof exported.createAutocompleteEditorExtensions, "function");
  assert.deepEqual(requested.filter((specifier) => specifier.startsWith(".")), []);
});
