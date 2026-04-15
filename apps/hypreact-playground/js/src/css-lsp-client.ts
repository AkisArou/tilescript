import * as monaco from "monaco-editor/esm/vs/editor/editor.api.js";
import CssLspWorker from "./css-lsp-worker?worker";

type LspRequestMessage = {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params?: unknown;
};

type LspNotificationMessage = {
  jsonrpc: "2.0";
  method: string;
  params?: unknown;
};

type LspResponseMessage = {
  jsonrpc: "2.0";
  id: number | string | null;
  result?: unknown;
  error?: { code: number; message: string };
};

type WorkerOutput = {
  response?: LspResponseMessage | null;
  events: Array<{ message: LspNotificationMessage }>;
};

type Diagnostic = {
  message: string;
  severity?: number;
  range: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
};

const worker = new CssLspWorker();
let nextRequestId = 1;
const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
let readyPromise: Promise<void> | null = null;
let initialized = false;

worker.onmessage = (event: MessageEvent<any>) => {
  const data = event.data;
  if (data.type === "ready") return;
  if (data.type === "error") {
    for (const entry of pending.values()) {
      entry.reject(new Error(data.error));
    }
    pending.clear();
    return;
  }
  if (data.type === "output") {
    handleOutput(JSON.parse(data.outputJson) as WorkerOutput);
  }
};

export async function ensureCssLspReady() {
  if (readyPromise) return readyPromise;

  readyPromise = new Promise<void>((resolve, reject) => {
    const handleMessage = (event: MessageEvent<any>) => {
      if (event.data?.type === "ready") {
        worker.removeEventListener("message", handleMessage);
        resolve();
      } else if (event.data?.type === "error") {
        worker.removeEventListener("message", handleMessage);
        reject(new Error(event.data.error));
      }
    };

    worker.addEventListener("message", handleMessage);
    worker.postMessage({ type: "init" });
  }).then(async () => {
    if (initialized) return;
    await initializeServer();
    initialized = true;
  });

  return readyPromise;
}

export async function syncCssDocument(model: monaco.editor.ITextModel) {
  await ensureCssLspReady();

  const uri = model.uri.toString();
  await notify({
    jsonrpc: "2.0",
    method: "textDocument/didOpen",
    params: {
      textDocument: {
        uri,
        languageId: model.getLanguageId(),
        version: model.getVersionId(),
        text: model.getValue(),
      },
    },
  });
}

export async function updateCssDocument(model: monaco.editor.ITextModel) {
  await ensureCssLspReady();

  await notify({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      textDocument: {
        uri: model.uri.toString(),
        version: model.getVersionId(),
      },
      contentChanges: [{ text: model.getValue() }],
    },
  });
}

export async function closeCssDocument(model: monaco.editor.ITextModel) {
  await ensureCssLspReady();

  await notify({
    jsonrpc: "2.0",
    method: "textDocument/didClose",
    params: {
      textDocument: {
        uri: model.uri.toString(),
      },
    },
  });
}

export async function requestCssHover(
  model: monaco.editor.ITextModel,
  position: monaco.Position,
) {
  return request("textDocument/hover", {
    textDocument: { uri: model.uri.toString() },
    position: { line: position.lineNumber - 1, character: position.column - 1 },
  });
}

export async function requestCssCompletion(
  model: monaco.editor.ITextModel,
  position: monaco.Position,
) {
  return request("textDocument/completion", {
    textDocument: { uri: model.uri.toString() },
    position: { line: position.lineNumber - 1, character: position.column - 1 },
  });
}

export async function requestCssDefinition(
  model: monaco.editor.ITextModel,
  position: monaco.Position,
) {
  return request("textDocument/definition", {
    textDocument: { uri: model.uri.toString() },
    position: { line: position.lineNumber - 1, character: position.column - 1 },
  });
}

export async function requestCssReferences(
  model: monaco.editor.ITextModel,
  position: monaco.Position,
) {
  return request("textDocument/references", {
    textDocument: { uri: model.uri.toString() },
    position: { line: position.lineNumber - 1, character: position.column - 1 },
    context: { includeDeclaration: true },
  });
}

export async function requestCssRename(
  model: monaco.editor.ITextModel,
  position: monaco.Position,
  newName: string,
) {
  return request("textDocument/rename", {
    textDocument: { uri: model.uri.toString() },
    position: { line: position.lineNumber - 1, character: position.column - 1 },
    newName,
  });
}

async function initializeServer() {
  const message: LspRequestMessage = {
    jsonrpc: "2.0",
    id: nextRequestId++,
    method: "initialize",
    params: {
      processId: null,
      clientInfo: { name: "hypreact-css-lsp-web-client" },
      capabilities: {},
      rootUri: null,
    },
  };

  await new Promise<void>((resolve, reject) => {
    pending.set(message.id, {
      resolve: () => resolve(),
      reject,
    });
    worker.postMessage({ type: "initialize", message: JSON.stringify(message) });
  });

  await notify({
    jsonrpc: "2.0",
    method: "initialized",
    params: {},
  });
}

async function request(method: string, params: unknown) {
  await ensureCssLspReady();

  const id = nextRequestId++;
  const message: LspRequestMessage = { jsonrpc: "2.0", id, method, params };

  return new Promise<unknown>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    worker.postMessage({ type: "message", message: JSON.stringify(message) });
  });
}

async function notify(message: LspNotificationMessage) {
  worker.postMessage({ type: "message", message: JSON.stringify(message) });
}

function handleOutput(output: WorkerOutput) {
  for (const event of output.events) {
    handleServerNotification(event.message);
  }

  const response = output.response;
  if (!response || typeof response.id !== "number") return;

  const pendingRequest = pending.get(response.id);
  if (!pendingRequest) return;
  pending.delete(response.id);

  if (response.error) {
    pendingRequest.reject(new Error(response.error.message));
  } else {
    pendingRequest.resolve(response.result);
  }
}

function handleServerNotification(message: LspNotificationMessage) {
  if (message.method !== "textDocument/publishDiagnostics") return;

  const params = message.params as {
    uri: string;
    diagnostics: Diagnostic[];
  };

  const model = monaco.editor.getModel(monaco.Uri.parse(params.uri));
  if (!model) return;

  monaco.editor.setModelMarkers(
    model,
    "hypreact-css-lsp",
    params.diagnostics.map((diagnostic) => ({
      message: diagnostic.message,
      severity: toMonacoSeverity(diagnostic.severity),
      startLineNumber: diagnostic.range.start.line + 1,
      startColumn: diagnostic.range.start.character + 1,
      endLineNumber: diagnostic.range.end.line + 1,
      endColumn: diagnostic.range.end.character + 1,
    })),
  );
}

function toMonacoSeverity(severity?: number) {
  switch (severity) {
    case 1:
      return monaco.MarkerSeverity.Error;
    case 2:
      return monaco.MarkerSeverity.Warning;
    case 3:
      return monaco.MarkerSeverity.Info;
    case 4:
      return monaco.MarkerSeverity.Hint;
    default:
      return monaco.MarkerSeverity.Error;
  }
}
