#pragma once

namespace hypreact_plugin {

struct HookCallbacks {
  bool (*drainLayoutRuntimeSourceChanges)();
  void (*loadLayoutRuntimeConfig)();
  bool (*layoutRuntimeLoaded)();
  void (*resyncAll)();
  void (*registerHypreactAlgorithm)();
  void (*refreshWorkspaceAlgorithms)();
};

void registerHooks(const HookCallbacks &callbacks);
void clearHooks();

} // namespace hypreact_plugin
