#pragma once

namespace hypreact_plugin {

struct HookCallbacks {
  void (*loadLayoutRuntimeConfig)();
  bool (*layoutRuntimeLoaded)();
  void (*registerHypreactAlgorithm)();
  void (*refreshWorkspaceAlgorithms)();
};

void registerHooks(const HookCallbacks &callbacks);
void clearHooks();

} // namespace hypreact_plugin
