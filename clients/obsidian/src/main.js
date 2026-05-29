"use strict";

const { Plugin, PluginSettingTab, Setting } = require("obsidian");
const { EditorView, Decoration, ViewPlugin, WidgetType, keymap } = require("@codemirror/view");
const { Prec } = require("@codemirror/state");
const { BrokerClient, TRIGGER_DOCUMENT_CHANGE, TRIGGER_IDLE } = require("../../protocol/src/client.js");
const {
  DEFAULT_OBSIDIAN_SETTINGS,
  ObsidianAutocompleteController,
  normalizeObsidianSettings,
} = require("./plugin-core.js");

class GhostTextWidget extends WidgetType {
  constructor(text) {
    super();
    this.text = text;
  }

  toDOM() {
    const span = document.createElement("span");
    span.className = "autocomplete-alternative-ghost-text";
    span.textContent = this.text;
    return span;
  }

  ignoreEvent() {
    return true;
  }
}

function decorationsForSuggestion(suggestion) {
  if (!suggestion) {
    return Decoration.none;
  }
  return Decoration.set([
    Decoration.widget({ widget: new GhostTextWidget(suggestion.text), side: 1 }).range(suggestion.from),
  ]);
}

function createAutocompleteEditorExtensions(plugin) {
  let viewPlugin;
  viewPlugin = ViewPlugin.fromClass(
    class AutocompleteAlternativeViewPlugin {
      constructor(view) {
        this.controller = new ObsidianAutocompleteController({
          settings: plugin.settings,
          client: plugin.brokerClient,
        });
        this.decorations = Decoration.none;
        this.requestOrdinal = 0;
        this.request(view, TRIGGER_IDLE);
      }

      update(update) {
        if (!update.docChanged && !update.selectionSet) {
          return;
        }
        this.controller.cancelStale("codemirror_update");
        this.decorations = Decoration.none;
        this.request(update.view, update.docChanged ? TRIGGER_DOCUMENT_CHANGE : TRIGGER_IDLE);
      }

      async request(view, trigger) {
        const ordinal = ++this.requestOrdinal;
        const suggestion = await this.controller.requestSuggestion({
          state: view.state,
          file: plugin.app.workspace.getActiveFile(),
          vaultName: plugin.app.vault.getName ? plugin.app.vault.getName() : "vault",
          trigger,
        });
        if (ordinal !== this.requestOrdinal) {
          return;
        }
        this.decorations = decorationsForSuggestion(suggestion);
        view.dispatch({});
      }

      accept(view) {
        const accepted = this.controller.accept(view);
        if (accepted) {
          this.decorations = Decoration.none;
          view.dispatch({});
        }
        return accepted;
      }
    },
    {
      decorations: (value) => value.decorations,
    },
  );

  const acceptWithTab = Prec.highest(
    keymap.of([
      {
        key: "Tab",
        run: (view) => {
          const pluginValue = view.plugin(viewPlugin);
          return pluginValue ? pluginValue.accept(view) : false;
        },
      },
    ]),
  );

  return [viewPlugin, acceptWithTab, ghostTextTheme()];
}

function ghostTextTheme() {
  return EditorView.theme({
    ".autocomplete-alternative-ghost-text": {
      opacity: "0.45",
      fontStyle: "italic",
      pointerEvents: "none",
    },
  });
}

class AutocompleteAlternativeSettingTab extends PluginSettingTab {
  constructor(app, plugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display() {
    const { containerEl } = this;
    containerEl.empty();

    new Setting(containerEl)
      .setName("Broker URL")
      .setDesc("Local autocomplete broker base URL.")
      .addText((text) => text
        .setValue(this.plugin.settings.brokerUrl)
        .onChange(async (value) => {
          this.plugin.settings.brokerUrl = value || DEFAULT_OBSIDIAN_SETTINGS.brokerUrl;
          this.plugin.brokerClient = new BrokerClient({ brokerUrl: this.plugin.settings.brokerUrl });
          await this.plugin.saveSettings();
        }));

    new Setting(containerEl)
      .setName("Debounce (ms)")
      .setDesc("Delay before sending an autocomplete request after note changes.")
      .addText((text) => text
        .setValue(String(this.plugin.settings.debounceMs))
        .onChange(async (value) => {
          this.plugin.settings.debounceMs = Number(value) || DEFAULT_OBSIDIAN_SETTINGS.debounceMs;
          await this.plugin.saveSettings();
        }));

    new Setting(containerEl)
      .setName("Max context characters")
      .setDesc("Maximum prefix plus suffix characters sent to the broker.")
      .addText((text) => text
        .setValue(String(this.plugin.settings.maxContextChars))
        .onChange(async (value) => {
          this.plugin.settings.maxContextChars = Number(value) || DEFAULT_OBSIDIAN_SETTINGS.maxContextChars;
          await this.plugin.saveSettings();
        }));
  }
}

class AutocompleteAlternativeObsidianPlugin extends Plugin {
  async onload() {
    this.settings = normalizeObsidianSettings(await this.loadData());
    this.brokerClient = new BrokerClient({ brokerUrl: this.settings.brokerUrl });
    this.registerEditorExtension(createAutocompleteEditorExtensions(this));
    this.addSettingTab(new AutocompleteAlternativeSettingTab(this.app, this));
  }

  async saveSettings() {
    await this.saveData(this.settings);
  }
}

module.exports = AutocompleteAlternativeObsidianPlugin;
module.exports.createAutocompleteEditorExtensions = createAutocompleteEditorExtensions;
