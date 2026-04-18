#include "tilescript_hypr_plugin_algorithm.hpp"

#include <optional>
#include <iostream>
#include <typeinfo>

#include "tilescript_hypr_plugin_runtime.hpp"

#include "src/layout/space/Space.hpp"
#include "src/layout/supplementary/WorkspaceAlgoMatcher.hpp"
#include "src/layout/target/Target.hpp"
#include "src/layout/algorithm/tiled/master/MasterAlgorithm.hpp"
#include "src/plugins/PluginAPI.hpp"

namespace tilescript_plugin {
namespace {

std::optional<AlgorithmCallbacks> g_algorithmCallbacks;
bool g_registeredTilescriptAlgo = false;

class CTilescriptAlgorithm final : public Layout::ITiledAlgorithm {
public:
  void newTarget(SP<Layout::ITarget> target) override { recalculate(); }

  void movedTarget(SP<Layout::ITarget> target,
                   std::optional<Vector2D> focalPoint = std::nullopt) override {
    recalculate();
  }

  void removeTarget(SP<Layout::ITarget> target) override { recalculate(); }

  void resizeTarget(const Vector2D &, SP<Layout::ITarget>,
                    Layout::eRectCorner = Layout::CORNER_NONE) override {}

  void recalculate() override {
    const auto parent = m_parent.lock();
    if (!parent || !runtime() || !g_algorithmCallbacks.has_value()) {
      return;
    }

    const auto space = parent->space();
    if (!space) {
      return;
    }

    const auto workspace = space->workspace();
    if (!workspace) {
      return;
    }

    const auto placement = runtime()->layoutPlacementForWorkspace(
        g_algorithmCallbacks->workspaceName(workspace));
    const auto byWindowId = geometryMapFromPlacement(placement);
    tilescript_runtime_free_placement_result(placement);

    for (const auto &weakTarget : space->targets()) {
      const auto target = weakTarget.lock();
      if (!target || target->floating() || !target->window()) {
        continue;
      }

      if (target->window()->m_workspace != workspace) {
        continue;
      }

      const auto windowId = g_algorithmCallbacks->makeWindowId(target->window());
      const auto it = byWindowId.find(windowId);
      if (it == byWindowId.end()) {
        continue;
      }

      if (it->second.w <= 0 || it->second.h <= 0) {
        continue;
      }

      const auto box = offsetPlacementToWorkspace(it->second, workspace);
      target->setPositionGlobal({.logicalBox = box, .visualBox = box});
    }
  }

  void swapTargets(SP<Layout::ITarget> a, SP<Layout::ITarget> b) override {
    recalculate();
  }

  void moveTargetInDirection(SP<Layout::ITarget> t, Math::eDirection dir,
                             bool silent) override {
    if (!t || !t->window() || !runtime() || !g_algorithmCallbacks.has_value()) {
      return;
    }

    const auto parent = m_parent.lock();
    if (!parent) {
      return;
    }

    const auto space = parent->space();
    if (!space) {
      return;
    }

    if (!runtime()->layoutMoveDirection(Math::toString(dir))) {
      return;
    }

    recalculate();
  }

  SP<Layout::ITarget> getNextCandidate(SP<Layout::ITarget> old) override {
    const auto parent = m_parent.lock();
    if (!parent || !old || !old->window() || !runtime() ||
        !g_algorithmCallbacks.has_value()) {
      return old;
    }

    const auto space = parent->space();
    if (!space) {
      return old;
    }

    const auto workspace = space->workspace();
    if (!workspace) {
      return old;
    }

    const auto candidateId = runtime()->layoutCloseFocusCandidate(
        g_algorithmCallbacks->makeWindowId(old->window()));
    if (!candidateId.has_value()) {
      return old;
    }

    for (const auto &weakTarget : space->targets()) {
      const auto target = weakTarget.lock();
      if (!target || target->floating() || !target->window()) {
        continue;
      }
      if (target->window()->m_workspace != workspace) {
        continue;
      }
      if (g_algorithmCallbacks->makeWindowId(target->window()) ==
          *candidateId) {
        return target;
      }
    }

    return old;
  }
};

} // namespace

std::unordered_map<std::string, CBox>
geometryMapFromPlacement(const TilescriptPlacementResult &placement) {
  std::unordered_map<std::string, CBox> byWindowId;
  byWindowId.reserve(placement.geometry_count);
  for (size_t i = 0; i < placement.geometry_count; ++i) {
    const auto &entry = placement.geometries[i];
    if (entry.window_id == nullptr) {
      continue;
    }

    byWindowId.emplace(entry.window_id, CBox{
                                            static_cast<double>(entry.x),
                                            static_cast<double>(entry.y),
                                            static_cast<double>(entry.width),
                                            static_cast<double>(entry.height),
                                        });
  }
  return byWindowId;
}

CBox offsetPlacementToWorkspace(const CBox &box,
                                const PHLWORKSPACE &workspace) {
  if (!workspace || !workspace->m_space) {
    return box;
  }

  const auto workArea = workspace->m_space->workArea(false);
  return CBox{
      workArea.x + box.x,
      workArea.y + box.y,
      box.w,
      box.h,
  };
}

void refreshWorkspaceAlgorithms() {
  if (!g_registeredTilescriptAlgo || !layoutRuntimeLoaded()) {
    return;
  }

  Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
}

void registerTilescriptAlgorithm(HANDLE pluginHandle,
                               const AlgorithmCallbacks &callbacks) {
  if (g_registeredTilescriptAlgo) {
    return;
  }

  g_algorithmCallbacks = callbacks;
  g_registeredTilescriptAlgo = HyprlandAPI::addTiledAlgo(
      pluginHandle, "tilescript", &typeid(CTilescriptAlgorithm),
      []() -> UP<Layout::ITiledAlgorithm> {
        return makeUnique<CTilescriptAlgorithm>();
      });

  if (g_registeredTilescriptAlgo) {
    std::cout << "[tilescript] registered tiled algorithm: tilescript" << std::endl;
  } else {
    std::cerr << "[tilescript] failed to register tiled algorithm: tilescript"
              << std::endl;
  }
}

void unregisterTilescriptAlgorithm(HANDLE pluginHandle) {
  if (!g_registeredTilescriptAlgo) {
    return;
  }

  if (!HyprlandAPI::removeAlgo(pluginHandle, "tilescript")) {
    std::cerr << "[tilescript] failed to unregister tiled algorithm: tilescript"
              << std::endl;
    return;
  }

  std::cout << "[tilescript] unregistered tiled algorithm: tilescript" << std::endl;
  g_registeredTilescriptAlgo = false;
  g_algorithmCallbacks.reset();
}

} // namespace tilescript_plugin
