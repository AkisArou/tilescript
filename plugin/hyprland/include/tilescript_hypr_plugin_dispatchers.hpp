#pragma once

#include <optional>
#include <string>

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/desktop/view/Window.hpp"
#include "src/helpers/Monitor.hpp"
#include "src/plugins/PluginAPI.hpp"

namespace tilescript_plugin {

struct DispatcherCallbacks {
  SDispatchResult (*callDispatcher)(const std::string &name,
                                    const std::string &arg);
  std::string (*makeWindowId)(const PHLWINDOW &window);
  void (*syncWorkspace)(const PHLWORKSPACE &workspace,
                        const PHLMONITOR &monitor);
  void (*syncWindow)(const PHLWINDOW &window);
  void (*syncWorkspaceWindows)(const PHLWORKSPACE &workspace);
  void (*recalculateWorkspace)(const PHLWORKSPACE &workspace);
  void (*syncFocusedWindow)(const PHLWINDOW &window);
  void (*queueWorkspaceRecalculate)(const PHLWORKSPACE &workspace);
  void (*applyPlacementForWorkspace)(const PHLWORKSPACE &workspace);
  void (*markRecentWorkspaceResize)(const PHLWORKSPACE &workspace);
};

void registerTilescriptDispatchers(HANDLE pluginHandle,
                                 const DispatcherCallbacks &callbacks);

} // namespace tilescript_plugin
