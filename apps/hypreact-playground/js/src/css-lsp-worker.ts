/// <reference lib="webworker" />

import init, { WasmServer } from "../generated/css-lsp-web/index.js";

type WorkerRequest =
  | { type: "init"; files?: Record<string, string> }
  | { type: "initialize"; message: string }
  | { type: "message"; message: string };

let server: WasmServer | null = null;

self.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const data = event.data;

  if (data.type === "init") {
    await init();
    server = data.files
      ? WasmServer.withFilesJson(JSON.stringify(data.files))
      : new WasmServer();
    self.postMessage({ type: "ready" });
    return;
  }

  if (!server) {
    self.postMessage({ type: "error", error: "css lsp worker not initialized" });
    return;
  }

  try {
    const outputJson = data.type === "initialize"
      ? server.handleInitializeJson(data.message)
      : server.handleMessageJson(data.message);

    self.postMessage({ type: "output", outputJson });
  } catch (error) {
    self.postMessage({
      type: "error",
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
