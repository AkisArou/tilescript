#pragma once

#include <string>
#include <unordered_map>

#include "hypreact_hypr_ffi.h"

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/desktop/view/Window.hpp"
#include "src/layout/algorithm/Algorithm.hpp"
#include "src/plugins/PluginAPI.hpp"

namespace hypreact_plugin {

struct AlgorithmCallbacks {
  std::string (*makeWindowId)(const PHLWINDOW &window);
  std::string (*workspaceName)(const PHLWORKSPACE &workspace);
};

std::unordered_map<std::string, CBox>
geometryMapFromPlacement(const HypreactPlacementResult &placement);

CBox offsetPlacementToWorkspace(const CBox &box, const PHLWORKSPACE &workspace);

void refreshWorkspaceAlgorithms();
void registerHypreactAlgorithm(HANDLE pluginHandle,
                               const AlgorithmCallbacks &callbacks);
void unregisterHypreactAlgorithm(HANDLE pluginHandle);

} // namespace hypreact_plugin
