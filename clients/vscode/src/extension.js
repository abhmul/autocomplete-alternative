"use strict";

const vscode = require("vscode");
const { BrokerClient } = require("../../protocol/src/client.js");
const { createInlineCompletionProvider, readVsCodeSettings } = require("./extension-core.js");

function activate(context) {
  const extensionVersion = context.extension && context.extension.packageJSON
    ? context.extension.packageJSON.version
    : "0.1.0";
  const settings = readVsCodeSettings(vscode, extensionVersion);
  const client = new BrokerClient({ brokerUrl: settings.brokerUrl });
  const provider = createInlineCompletionProvider({ vscode, settings, client });

  context.subscriptions.push(provider);
  context.subscriptions.push(
    vscode.languages.registerInlineCompletionItemProvider({ pattern: "**" }, provider),
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
