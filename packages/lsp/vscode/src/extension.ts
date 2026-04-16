import { access } from "node:fs/promises";
import path from "node:path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
  Trace,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand("tilescriptCss.restartServer", async () => {
      await restartClient(context);
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (!event.affectsConfiguration("tilescriptCss.enable")) {
        return;
      }

      if (isExtensionEnabled()) {
        await startClient(context);
      } else {
        await deactivate();
      }
    }),
  );

  if (isExtensionEnabled()) {
    await startClient(context);
  }
}

export async function deactivate(): Promise<void> {
  if (!client) {
    return;
  }

  const activeClient = client;
  client = undefined;
  await activeClient.stop();
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  await deactivate();
  await startClient(context);
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  if (client) {
    return;
  }

  if (!isExtensionEnabled()) {
    return;
  }

  const command = await resolveServerCommand(context);
  if (!command) {
    void vscode.window.showWarningMessage(
      "Could not find `tilescript-css-lsp`. Set `tilescriptCss.server.path`, use the bundled binary, or build the Rust server first.",
    );
    return;
  }

  const serverOptions: ServerOptions = {
    command,
    args: [],
    options: {
      cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "css", scheme: "file" }],
    synchronize: {
      configurationSection: "tilescriptCss",
    },
    outputChannelName: "Tilescript CSS Language Server",
    revealOutputChannelOn: RevealOutputChannelOn.Never,
  };

  client = new LanguageClient(
    "tilescript-css-lsp",
    "Tilescript CSS Language Server",
    serverOptions,
    clientOptions,
  );

  client.setTrace(toTrace(vscode.workspace.getConfiguration("tilescriptCss").get("server.trace")));
  await client.start();
}

async function resolveServerCommand(
  context: vscode.ExtensionContext,
): Promise<string | undefined> {
  const configuredPath = vscode.workspace
    .getConfiguration("tilescriptCss")
    .get<string>("server.path")
    ?.trim();

  if (configuredPath) {
    return configuredPath;
  }

  const bundledCommand = await resolveBundledServerCommand(context);
  if (bundledCommand) {
    return bundledCommand;
  }

  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!workspaceRoot) {
    return undefined;
  }

  const candidates = [
    path.join(workspaceRoot, "target", "debug", "tilescript-css-lsp"),
    path.join(workspaceRoot, "target", "release", "tilescript-css-lsp"),
  ];

  for (const candidate of candidates) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      continue;
    }
  }

  return undefined;
}

async function resolveBundledServerCommand(
  context: vscode.ExtensionContext,
): Promise<string | undefined> {
  if (process.platform !== "linux" || process.arch !== "x64") {
    return undefined;
  }

  const bundledPath = path.join(context.extensionPath, "server", "linux-x64", "tilescript-css-lsp");

  try {
    await access(bundledPath);
    return bundledPath;
  } catch {
    return undefined;
  }
}

function toTrace(value: string | undefined): Trace {
  switch (value) {
    case "messages":
      return Trace.Messages;
    case "verbose":
      return Trace.Verbose;
    default:
      return Trace.Off;
  }
}

function isExtensionEnabled(): boolean {
  return vscode.workspace.getConfiguration("tilescriptCss").get<boolean>("enable", false);
}
