import { LuaFactory } from "wasmoon";

type LayoutDependencies = {
  usesMonitorSize: boolean;
  usesMonitorScale: boolean;
  usesWindowCount: boolean;
  usesWindowOrder: boolean;
  usesWindowFocus: boolean;
  usesVisibleWindowIds: boolean;
  usesWorkspaceName: boolean;
  usesWorkspaceNames: boolean;
  usesSelectedLayoutName: boolean;
  usesLayoutAdjustments: boolean;
};

type LayoutEvaluationResult = {
  layout: unknown;
  dependencies: LayoutDependencies;
};

const factory = new LuaFactory();

function defaultDependencies(): LayoutDependencies {
  return {
    usesMonitorSize: false,
    usesMonitorScale: false,
    usesWindowCount: false,
    usesWindowOrder: false,
    usesWindowFocus: false,
    usesVisibleWindowIds: false,
    usesWorkspaceName: false,
    usesWorkspaceNames: false,
    usesSelectedLayoutName: false,
    usesLayoutAdjustments: false,
  };
}

function createTrackedLayoutContext(context: any) {
  const dependencies = defaultDependencies();

  const trackedWindows = context.windows.map((window: any) =>
    new Proxy(window, {
      get(target, prop, receiver) {
        if (prop === "focused") {
          dependencies.usesWindowFocus = true;
        } else if (typeof prop === "string" && prop !== "id") {
          dependencies.usesWindowOrder = true;
        }
        return Reflect.get(target, prop, receiver);
      },
    }),
  );

  const windowsProxy = new Proxy(trackedWindows, {
    get(target, prop, receiver) {
      if (prop === "length") {
        dependencies.usesWindowCount = true;
        return Reflect.get(target, prop, receiver);
      }

      if (typeof prop === "string") {
        const index = Number(prop);
        if (!Number.isNaN(index)) {
          dependencies.usesWindowOrder = true;
        }
      }

      return Reflect.get(target, prop, receiver);
    },
  });

  const trackedWorkspace = new Proxy(context.workspace, {
    get(target, prop, receiver) {
      if (prop === "windowCount") {
        dependencies.usesWindowCount = true;
      } else if (prop === "name") {
        dependencies.usesWorkspaceName = true;
      } else if (prop === "workspaces") {
        dependencies.usesWorkspaceNames = true;
      }
      return Reflect.get(target, prop, receiver);
    },
  });

  const trackedMonitor = new Proxy(context.monitor, {
    get(target, prop, receiver) {
      if (prop === "width" || prop === "height") {
        dependencies.usesMonitorSize = true;
      } else if (prop === "scale") {
        dependencies.usesMonitorScale = true;
      }
      return Reflect.get(target, prop, receiver);
    },
  });

  const trackedState = context.state
    ? new Proxy(context.state, {
        get(target, prop, receiver) {
          if (prop === "focusedWindowId") {
            dependencies.usesWindowFocus = true;
          } else if (prop === "visibleWindowIds") {
            dependencies.usesVisibleWindowIds = true;
          } else if (prop === "selectedLayoutName") {
            dependencies.usesSelectedLayoutName = true;
          } else if (prop === "resizeState") {
            dependencies.usesLayoutAdjustments = true;
          } else if (prop === "workspaceNames") {
            dependencies.usesWorkspaceNames = true;
          }
          return Reflect.get(target, prop, receiver);
        },
      })
    : undefined;

  return {
    context: new Proxy(context, {
      get(target, prop, receiver) {
        if (prop === "windows") return windowsProxy;
        if (prop === "workspace") return trackedWorkspace;
        if (prop === "monitor") return trackedMonitor;
        if (prop === "state") return trackedState;
        return Reflect.get(target, prop, receiver);
      },
    }),
    dependencies,
  };
}

async function withLuaEngine<T>(
  sdkSource: string,
  callback: (engine: Awaited<ReturnType<LuaFactory["createEngine"]>>) => Promise<T>,
): Promise<T> {
  const engine = await factory.createEngine({
    injectObjects: true,
    enableProxy: true,
  });

  try {
    engine.global.set("__hypreact_sdk_source", sdkSource);
    await engine.doString(`
      local hypreact = assert(load(__hypreact_sdk_source, "@hypreact-sdk"))()
      package.preload["hypreact"] = function()
        return hypreact
      end
    `);
    return await callback(engine);
  } finally {
    engine.global.close();
  }
}

export async function evaluateLuaConfig(
  source: string,
  chunkName: string,
  sdkSource: string,
) {
  return withLuaEngine(sdkSource, async (engine) => {
    return await engine.doString(`
      local result = assert(load(${JSON.stringify(source)}, ${JSON.stringify(chunkName)}))()
      return result
    `);
  });
}

export async function evaluateLuaLayout(
  source: string,
  chunkName: string,
  sdkSource: string,
  context: unknown,
): Promise<LayoutEvaluationResult> {
  return withLuaEngine(sdkSource, async (engine) => {
    const tracked = createTrackedLayoutContext(context);
    engine.global.set("__hypreact_context", tracked.context);
    const layout = await engine.doString(`
      local layout = assert(load(${JSON.stringify(source)}, ${JSON.stringify(chunkName)}))()
      return layout(__hypreact_context)
    `);

    return {
      layout,
      dependencies: tracked.dependencies,
    };
  });
}
