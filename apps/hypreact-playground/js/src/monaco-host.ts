// oxlint-disable import/default
import "monaco-editor/min/vs/editor/editor.main.css";
import "monaco-editor/esm/vs/editor/editor.main.js";
import * as monaco from "monaco-editor";
import "monaco-editor/esm/vs/language/css/monaco.contribution.js";
import "monaco-editor/esm/vs/language/typescript/monaco.contribution.js";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import CssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import TsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";
import {
  closeCssDocument,
  requestCssCompletion,
  requestCssDefinition,
  requestCssHover,
  requestCssReferences,
  requestCssRename,
  syncCssDocument,
  updateCssDocument,
} from "./css-lsp-client.js";

interface MonacoModel {
  path: string;
  language: string;
  value: string;
}

interface MonacoExtraLib {
  filePath: string;
  content: string;
}

interface MonacoHostHandle {
  host: HTMLElement;
  monaco: typeof monaco;
  editor: monaco.editor.IStandaloneCodeEditor;
  modelPaths: string[];
  activePath: string | null;
  sourceLibs: Map<string, { content: string; dispose: monaco.IDisposable }>;
  changeDisposable?: monaco.IDisposable;
  openerDisposable?: monaco.IDisposable;
}

const monacoTheme = "hypreact-terminal";
const workspaceRootUri = "file:///home/demo/.config/hypreact";

self.MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    switch (label) {
      case "css":
        return new CssWorker();
      case "typescript":
      case "javascript":
        return new TsWorker();
      default:
        return new EditorWorker();
    }
  },
};

let configured = false;
let cssLspRegistered = false;

function ensureMonacoStyles() {
  const styleId = "hypreact-monaco-host-css";
  if (document.getElementById(styleId)) return;

  const link = document.createElement("link");
  link.id = styleId;
  link.rel = "stylesheet";
  link.href = "/monaco/hypreact-playground-monaco-host.css";
  document.head.appendChild(link);
}

function ensureConfigured(extraLibs: MonacoExtraLib[]) {
  ensureMonacoStyles();

  if (!configured) {
    monaco.typescript.javascriptDefaults.setEagerModelSync(true);

    monaco.typescript.typescriptDefaults.setCompilerOptions({
      allowJs: true,
      allowImportingTsExtensions: true,
      allowNonTsExtensions: true,
      allowSyntheticDefaultImports: true,
      baseUrl: workspaceRootUri,
      esModuleInterop: true,
      jsx: monaco.typescript.JsxEmit.ReactJSX,
      jsxImportSource: "@hypreact/sdk",
      module: monaco.typescript.ModuleKind.ESNext,
      moduleResolution: monaco.typescript.ModuleResolutionKind.NodeJs,
      paths: {
        "@hypreact/sdk": ["./node_modules/@hypreact/sdk/index.d.ts"],
        "@hypreact/sdk/*": ["./node_modules/@hypreact/sdk/*"],
      },
      target: monaco.typescript.ScriptTarget.ESNext,
    });

    monaco.typescript.typescriptDefaults.setDiagnosticsOptions({
      noSemanticValidation: false,
      noSyntaxValidation: false,
    });

    monaco.typescript.typescriptDefaults.setEagerModelSync(true);

    monaco.css.cssDefaults.setModeConfiguration({
      completionItems: false,
      colors: false,
      diagnostics: false,
      documentFormattingEdits: false,
      documentHighlights: false,
      documentRangeFormattingEdits: false,
      documentSymbols: false,
      foldingRanges: false,
      hovers: false,
      references: false,
      rename: false,
      selectionRanges: false,
    });

    for (const lib of extraLibs) {
      monaco.typescript.typescriptDefaults.addExtraLib(
        lib.content,
        lib.filePath,
      );
    }

    monaco.editor.defineTheme(monacoTheme, {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "comment", foreground: "6A9955" },
        { token: "keyword", foreground: "569CD6" },
        { token: "string", foreground: "CE9178" },
        { token: "number", foreground: "B5CEA8" },
        { token: "type.identifier", foreground: "4EC9B0" },
        { token: "delimiter", foreground: "D4D4D4" },
      ],
      colors: {
        "editor.background": "#1F1F1F",
        "editor.foreground": "#D4D4D4",
      },
    });

    configured = true;
  }

  if (!cssLspRegistered) {
    registerCssLspProviders();
    cssLspRegistered = true;
  }
}

function registerCssLspProviders() {
  monaco.languages.registerHoverProvider("css", {
    async provideHover(model, position) {
      const result = await requestCssHover(model, position);
      const hover = result as {
        contents?:
          | { kind?: string; value?: string }
          | Array<{ value?: string }>;
        range?: {
          start: { line: number; character: number };
          end: { line: number; character: number };
        };
      } | null;
      if (!hover?.contents) return null;

      const contents = Array.isArray(hover.contents)
        ? hover.contents.map((item) => ({ value: item.value ?? "" }))
        : [{ value: hover.contents.value ?? "" }];

      return {
        contents,
        range: hover.range
          ? new monaco.Range(
              hover.range.start.line + 1,
              hover.range.start.character + 1,
              hover.range.end.line + 1,
              hover.range.end.character + 1,
            )
          : undefined,
      };
    },
  });

  monaco.languages.registerCompletionItemProvider("css", {
    triggerCharacters: ["-", ":", ".", "#"],
    async provideCompletionItems(model, position) {
      const result = await requestCssCompletion(model, position);
      const response = result as { items?: any[] } | any[] | null;
      const items = Array.isArray(response)
        ? response
        : (response?.items ?? []);
      return {
        suggestions: items.map((item) => ({
          label: item.label,
          kind: monaco.languages.CompletionItemKind.Property,
          insertText: item.insertText ?? item.label,
          detail: item.detail,
          documentation:
            typeof item.documentation === "string"
              ? item.documentation
              : item.documentation?.value,
          range: new monaco.Range(
            position.lineNumber,
            position.column,
            position.lineNumber,
            position.column,
          ),
        })),
      };
    },
  });

  monaco.languages.registerDefinitionProvider("css", {
    async provideDefinition(model, position) {
      const result = await requestCssDefinition(model, position);
      return toMonacoLocations(result);
    },
  });

  monaco.languages.registerReferenceProvider("css", {
    async provideReferences(model, position) {
      const result = await requestCssReferences(model, position);
      return toMonacoLocations(result);
    },
  });

  monaco.languages.registerRenameProvider("css", {
    async provideRenameEdits(model, position, newName) {
      const result = await requestCssRename(model, position, newName);
      const edit = result as {
        changes?: Record<string, Array<{ range: any; newText: string }>>;
      } | null;

      const edits: monaco.languages.IWorkspaceTextEdit[] = [];
      for (const [uri, textEdits] of Object.entries(edit?.changes ?? {})) {
        for (const textEdit of textEdits) {
          edits.push({
            resource: monaco.Uri.parse(uri),
            textEdit: {
              range: new monaco.Range(
                textEdit.range.start.line + 1,
                textEdit.range.start.character + 1,
                textEdit.range.end.line + 1,
                textEdit.range.end.character + 1,
              ),
              text: textEdit.newText,
            },
            versionId: undefined,
          });
        }
      }

      return { edits };
    },
    resolveRenameLocation() {
      return null;
    },
  });
}

function toMonacoLocations(result: unknown) {
  const locations = (
    Array.isArray(result) ? result : result ? [result] : []
  ) as Array<{
    uri?: string;
    range?: {
      start: { line: number; character: number };
      end: { line: number; character: number };
    };
  }>;

  return locations
    .filter((location) => location.uri && location.range)
    .map((location) => ({
      uri: monaco.Uri.parse(location.uri!),
      range: new monaco.Range(
        location.range!.start.line + 1,
        location.range!.start.character + 1,
        location.range!.end.line + 1,
        location.range!.end.character + 1,
      ),
    }));
}

function syncModels(handle: MonacoHostHandle, models: MonacoModel[]) {
  handle.modelPaths = models.map((model) => model.path);
  const nextTypeScriptPaths = new Set<string>();

  for (const model of models) {
    if (model.language === "typescript") {
      nextTypeScriptPaths.add(model.path);
      const existingSourceLib = handle.sourceLibs.get(model.path);
      if (!existingSourceLib || existingSourceLib.content !== model.value) {
        existingSourceLib?.dispose.dispose();
        handle.sourceLibs.set(model.path, {
          content: model.value,
          dispose: monaco.typescript.typescriptDefaults.addExtraLib(
            model.value,
            model.path,
          ),
        });
      }
    }

    const uri = monaco.Uri.parse(model.path);
    const existingModel = monaco.editor.getModel(uri);
    if (existingModel && existingModel.getValue() !== model.value) {
      existingModel.setValue(model.value);
      if (model.language === "css") {
        void updateCssDocument(existingModel);
      }
    }
    if (existingModel && model.language === "css") {
      void syncCssDocument(existingModel);
    }
  }

  for (const [path, sourceLib] of handle.sourceLibs.entries()) {
    if (!nextTypeScriptPaths.has(path)) {
      sourceLib.dispose.dispose();
      handle.sourceLibs.delete(path);
    }
  }
}

function setActiveModel(handle: MonacoHostHandle, activePath: string | null) {
  if (!activePath) return;
  const uri = monaco.Uri.parse(activePath);
  const model = monaco.editor.getModel(uri);
  if (model && handle.editor.getModel() !== model) {
    handle.editor.setModel(model);
  }
}

export async function createMonacoEditor(
  host: HTMLElement,
  activePath: string,
  models: MonacoModel[],
  extraLibs: MonacoExtraLib[],
  onChange: (path: string, value: string) => void,
  onOpen: (payload: string) => void,
) {
  ensureConfigured(extraLibs);

  const activeModel = models.find((model) => model.path === activePath) ?? null;
  const initialValue = activeModel?.value ?? "";
  const initialLanguage = activeModel?.language ?? "typescript";
  const initialUri = activePath ? monaco.Uri.parse(activePath) : null;
  let fileBackedModel = initialUri ? monaco.editor.getModel(initialUri) : null;
  if (!fileBackedModel && initialUri) {
    fileBackedModel = monaco.editor.createModel(
      initialValue,
      initialLanguage,
      initialUri,
    );
  }

  const editor = monaco.editor.create(host, {
    automaticLayout: true,
    contextmenu: true,
    definitionLinkOpensInPeek: false,
    fontFamily:
      '"JetBrainsMono Nerd Font", "Symbols Nerd Font Mono", "IBM Plex Mono", monospace',
    fontLigatures: false,
    fontSize: 14,
    glyphMargin: false,
    gotoLocation: {
      multipleDeclarations: "peek",
      multipleDefinitions: "peek",
      multipleImplementations: "peek",
      multipleReferences: "peek",
      multipleTypeDefinitions: "peek",
    },
    lineHeight: 20,
    suggest: {
      insertMode: "replace",
      localityBonus: true,
      showSnippets: false,
      showKeywords: true,
    },
    inlineSuggest: {
      enabled: true,
    },
    quickSuggestions: {
      strings: "on",
    },
    suggestSelection: "first",
    hover: { enabled: true },
    bracketPairColorization: { enabled: true },
    linkedEditing: true,
    formatOnPaste: true,
    wordBasedSuggestions: "off",
    minimap: { enabled: false },
    padding: { top: 8, bottom: 8 },
    renderLineHighlight: "line",
    roundedSelection: false,
    scrollBeyondLastLine: false,
    smoothScrolling: false,
    tabSize: 2,
    theme: monacoTheme,
    wordWrap: "off",
  });
  editor.updateOptions({ editContext: false });
  if (fileBackedModel) {
    editor.setModel(fileBackedModel);
    if (fileBackedModel.getLanguageId() === "css") {
      void syncCssDocument(fileBackedModel);
    }
  }

  const handle: MonacoHostHandle = {
    host,
    monaco,
    editor,
    modelPaths: [],
    activePath: activePath || null,
    sourceLibs: new Map(),
  };

  syncModels(handle, models);
  setActiveModel(handle, activePath || null);

  handle.changeDisposable = editor.onDidChangeModelContent(() => {
    const model = editor.getModel();
    if (model) {
      onChange(model.uri.toString(), model.getValue());
      if (model.getLanguageId() === "css") {
        void updateCssDocument(model);
      }
    }
  });
  handle.openerDisposable = monaco.editor.registerEditorOpener({
    openCodeEditor(_source, resource, selectionOrPosition) {
      onOpen(
        JSON.stringify({
          path: resource.toString(),
          selectionOrPosition: selectionOrPosition ?? null,
        }),
      );
      return true;
    },
  });

  return handle;
}

export function updateMonacoEditor(
  handle: MonacoHostHandle,
  activePath: string,
  models: MonacoModel[],
  fontSize: number,
) {
  handle.activePath = activePath || null;
  syncModels(handle, models);
  setActiveModel(handle, activePath || null);
  handle.editor.updateOptions({
    fontSize,
    lineHeight: Math.round(fontSize * 1.45),
  });
}

export function revealMonacoPosition(
  handle: MonacoHostHandle,
  lineNumber: number,
  column: number,
) {
  const position = { lineNumber, column };
  handle.editor.setPosition(position);
  handle.editor.revealPositionInCenter(position);
  handle.editor.focus();
}

export function monacoMarkerCount(handle: MonacoHostHandle) {
  const model = handle.editor.getModel();
  if (!model) return 0;
  return handle.monaco.editor.getModelMarkers({ resource: model.uri }).length;
}

export function disposeMonacoEditor(handle: MonacoHostHandle) {
  for (const path of handle.modelPaths) {
    const model = monaco.editor.getModel(monaco.Uri.parse(path));
    if (model?.getLanguageId() === "css") {
      void closeCssDocument(model);
    }
  }
  handle.changeDisposable?.dispose();
  handle.openerDisposable?.dispose();
  handle.editor.dispose();
  for (const path of handle.modelPaths) {
    monaco.editor.getModel(monaco.Uri.parse(path))?.dispose();
  }
  for (const sourceLib of handle.sourceLibs.values()) {
    sourceLib.dispose.dispose();
  }
}
