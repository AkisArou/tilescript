#pragma once

namespace tilescript_plugin {

struct HookCallbacks {
  bool (*drainLayoutRuntimeSourceChanges)();
  void (*loadLayoutRuntimeConfig)();
  bool (*layoutRuntimeLoaded)();
  void (*resyncAll)();
  void (*registerTilescriptAlgorithm)();
  void (*refreshWorkspaceAlgorithms)();
};

void registerHooks(const HookCallbacks &callbacks);
void clearHooks();

} // namespace tilescript_plugin
