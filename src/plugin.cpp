#include <iostream>
#include <optional>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

#include <json/json.h>

#include "hypreact_hypr_ffi.h"
#include "hypreact_plugin_runtime.hpp"

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/desktop/state/FocusState.hpp"
#include "src/desktop/view/Window.hpp"
#include "src/event/EventBus.hpp"
#include "src/helpers/Monitor.hpp"
#include "src/layout/algorithm/Algorithm.hpp"
#include "src/layout/algorithm/tiled/master/MasterAlgorithm.hpp"
#include "src/layout/space/Space.hpp"
#include "src/layout/supplementary/WorkspaceAlgoMatcher.hpp"
#include "src/layout/target/Target.hpp"
#include "src/managers/KeybindManager.hpp"
#include "src/plugins/PluginAPI.hpp"

inline HANDLE PHANDLE = nullptr;

namespace {

using hypreact_plugin::clearConfigPathValue;
using hypreact_plugin::clearPluginHandle;
using hypreact_plugin::createRuntime;
using hypreact_plugin::destroyRuntime;
using hypreact_plugin::layoutRuntimeLoaded;
using hypreact_plugin::loadLayoutRuntimeConfig;
using hypreact_plugin::logJson;
using hypreact_plugin::parseJson;
using hypreact_plugin::Runtime;
using hypreact_plugin::runtime;
using hypreact_plugin::setConfigPathValue;
using hypreact_plugin::setPluginHandle;
using hypreact_plugin::stringify;
using hypreact_plugin::trim;

std::vector<CHyprSignalListener> g_listeners;
SP<SHyprCtlCommand> g_queryCommand;
std::unordered_map<WINDOWID, std::string> g_windowIds;
struct PendingWorkspaceRecalculation {
  PHLWORKSPACE workspace;
  int remainingTicks;
};

struct RecentWorkspaceResize {
  PHLWORKSPACE workspace;
  int remainingTicks;
};

struct WindowSyncPayload {
  std::string windowId;
  std::string workspaceId;
  std::string outputId;
  HypreactWindowSync ffi;
};

struct OutputSyncPayload {
  std::string outputId;
  std::string name;
  HypreactOutputSync ffi;
};

struct WorkspaceLayoutSpaceSyncPayload {
  std::string workspaceId;
  std::string outputId;
  HypreactWorkspaceLayoutSpaceSync ffi;
};

std::vector<PendingWorkspaceRecalculation> g_pendingWorkspaceRecalculations;
std::vector<RecentWorkspaceResize> g_recentWorkspaceResizes;
int g_pendingWorkspaceLayoutRefreshTicks = 0;
bool g_registeredHypreactAlgo = false;

std::vector<std::string> splitWords(const std::string &value) {
  std::istringstream stream(value);
  std::vector<std::string> words;
  std::string word;
  while (stream >> word) {
    words.push_back(word);
  }
  return words;
}

SDispatchResult callDispatcher(const std::string &name, const std::string &arg);
std::string makeWindowId(const PHLWINDOW &window);
std::string workspaceName(const PHLWORKSPACE &workspace);
void syncWorkspace(const PHLWORKSPACE &workspace, const PHLMONITOR &monitor);
void syncWindow(const PHLWINDOW &window);
void syncWorkspaceWindows(const PHLWORKSPACE &workspace);
void removeWindow(const PHLWINDOW &window);
void recalculateWorkspace(const PHLWORKSPACE &workspace);
void syncFocusedWindow(const PHLWINDOW &window);
void syncWorkspaceLayoutSpace(const PHLWORKSPACE &workspace);
void queueWorkspaceRecalculate(const PHLWORKSPACE &workspace);
void applyPlacementForWorkspace(const PHLWORKSPACE &workspace);
void flushPendingWorkspaceRecalculations();
void syncActiveRuntimeState();
void resyncAll();
void markRecentWorkspaceResize(const PHLWORKSPACE &workspace);
bool isWorkspaceInRecentResizeWindow(const PHLWORKSPACE &workspace);
void refreshWorkspaceAlgorithms();

std::optional<std::string> normalizeDirection(const std::string &arg) {
  const auto value = trim(arg);
  if (value == "l" || value == "left") {
    return "left";
  }
  if (value == "r" || value == "right") {
    return "right";
  }
  if (value == "u" || value == "up") {
    return "up";
  }
  if (value == "d" || value == "down") {
    return "down";
  }
  return std::nullopt;
}

PHLWINDOW windowFromHypreactId(const std::string &windowId) {
  for (const auto &window : g_pCompositor->m_windows) {
    if (!window || !window->m_isMapped) {
      continue;
    }

    if (makeWindowId(window) == windowId) {
      return window;
    }
  }

  return nullptr;
}

std::string fromFfiDirection(HypreactDirection direction) {
  switch (direction) {
  case HYPREACT_DIRECTION_LEFT:
    return "left";
  case HYPREACT_DIRECTION_RIGHT:
    return "right";
  case HYPREACT_DIRECTION_UP:
    return "up";
  case HYPREACT_DIRECTION_DOWN:
    return "down";
  }

  return "left";
}

SDispatchResult hypreactMoveFocusDispatcher(std::string arg) {
  if (!runtime()) {
    return {.success = false, .error = "runtime not initialized"};
  }

  const auto direction = normalizeDirection(arg);
  if (!direction.has_value()) {
    return {.success = false, .error = "invalid direction"};
  }

  const auto focusedWindow = Desktop::focusState()->window();
  if (focusedWindow) {
    syncWindow(focusedWindow);
    syncWorkspace(focusedWindow->m_workspace, focusedWindow->m_monitor.lock());
    syncFocusedWindow(focusedWindow);
  }

  const auto target = runtime()->layoutFocusCandidate(*direction);
  if (!target.has_value()) {
    return {};
  }

  const auto targetWindow = windowFromHypreactId(*target);
  if (!targetWindow) {
    return {.success = false, .error = "target window not found"};
  }

  std::ostringstream address;
  address << "address:0x" << std::hex
          << reinterpret_cast<uintptr_t>(targetWindow.get());
  return callDispatcher("focuswindow", address.str());
}

SDispatchResult hypreactMoveWindowDispatcher(std::string arg) {
  if (!runtime()) {
    return {.success = false, .error = "runtime not initialized"};
  }

  const auto direction = normalizeDirection(arg);
  if (!direction.has_value()) {
    return {.success = false, .error = "invalid direction"};
  }

  const auto focusedWindow = Desktop::focusState()->window();
  if (!focusedWindow) {
    return {.success = false, .error = "no focused window"};
  }
  if (focusedWindow->isFullscreen()) {
    return {.success = false, .error = "window is fullscreen"};
  }
  if (focusedWindow->m_isFloating) {
    return callDispatcher("movewindow", direction->substr(0, 1));
  }

  syncWindow(focusedWindow);
  syncWorkspace(focusedWindow->m_workspace, focusedWindow->m_monitor.lock());
  syncWorkspaceWindows(focusedWindow->m_workspace);
  syncFocusedWindow(focusedWindow);

  const auto candidateId = runtime()->layoutSwapCandidate(*direction);
  if (!candidateId.has_value()) {
    return {};
  }

  const auto candidateWindow = windowFromHypreactId(*candidateId);
  if (!candidateWindow) {
    return {.success = false, .error = "target window not found"};
  }

  const auto focusedWindowId = makeWindowId(focusedWindow);
  if (!runtime()->moveTiledWindow(focusedWindowId, *candidateId)) {
    return {.success = false, .error = "failed to move tiled window"};
  }

  const auto workspace = focusedWindow->m_workspace;
  if (workspace) {
    recalculateWorkspace(workspace);
  }

  return {};
}

SDispatchResult hypreactResizeWindowDispatcher(std::string arg) {
  if (!runtime()) {
    return {.success = false, .error = "runtime not initialized"};
  }

  const auto direction = normalizeDirection(arg);
  if (!direction.has_value()) {
    return {.success = false, .error = "invalid direction"};
  }

  const auto focusedWindow = Desktop::focusState()->window();
  if (!focusedWindow) {
    return {.success = false, .error = "no focused window"};
  }
  if (focusedWindow->isFullscreen()) {
    return {.success = false, .error = "window is fullscreen"};
  }
  if (focusedWindow->m_isFloating) {
    return callDispatcher("resizeactive",
                          direction->substr(0, 1) + std::string(" 40"));
  }

  syncWindow(focusedWindow);
  syncWorkspace(focusedWindow->m_workspace, focusedWindow->m_monitor.lock());
  syncWorkspaceWindows(focusedWindow->m_workspace);
  syncFocusedWindow(focusedWindow);

  if (!runtime()->resizeDirection(*direction)) {
    return {.success = false, .error = "no resize candidate"};
  }

  const auto workspace = focusedWindow->m_workspace;
  if (workspace) {
    applyPlacementForWorkspace(workspace);
    markRecentWorkspaceResize(workspace);
    if (workspace->m_space) {
      workspace->m_space->recalculate();
    }
    queueWorkspaceRecalculate(workspace);
  }

  return {};
}

void registerHypreactDispatchers() {
  HyprlandAPI::addDispatcherV2(PHANDLE, "hypreact:movefocus",
                               hypreactMoveFocusDispatcher);
  HyprlandAPI::addDispatcherV2(PHANDLE, "hypreact:movewindow",
                               hypreactMoveWindowDispatcher);
  HyprlandAPI::addDispatcherV2(PHANDLE, "hypreact:resizewindow",
                               hypreactResizeWindowDispatcher);
}

std::unordered_map<std::string, CBox>
geometryMapFromPlacement(const HypreactPlacementResult &placement) {
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

class CHypreactAlgorithm final : public Layout::ITiledAlgorithm {
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
    if (!parent || !runtime()) {
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

    const auto placement =
        runtime()->layoutPlacementForWorkspace(workspaceName(workspace));
    const auto byWindowId = geometryMapFromPlacement(placement);
    hypreact_runtime_free_placement_result(placement);

    for (const auto &weakTarget : space->targets()) {
      const auto target = weakTarget.lock();
      if (!target || target->floating() || !target->window()) {
        continue;
      }

      if (target->window()->m_workspace != workspace) {
        continue;
      }

      const auto windowId = makeWindowId(target->window());
      const auto it = byWindowId.find(windowId);
      if (it == byWindowId.end()) {
        continue;
      }

      if (it->second.w <= 0 || it->second.h <= 0) {
        continue;
      }

      target->setPositionGlobal(
          offsetPlacementToWorkspace(it->second, workspace));
    }
  }

  void swapTargets(SP<Layout::ITarget> a, SP<Layout::ITarget> b) override {
    recalculate();
  }

  void moveTargetInDirection(SP<Layout::ITarget> t, Math::eDirection dir,
                             bool silent) override {
    if (!t || !t->window() || !runtime()) {
      return;
    }

    const auto candidateId =
        runtime()->layoutSwapCandidate(Math::toString(dir));
    if (!candidateId.has_value()) {
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

    for (const auto &weakTarget : space->targets()) {
      const auto candidate = weakTarget.lock();
      if (!candidate || candidate == t || candidate->floating() ||
          !candidate->window()) {
        continue;
      }

      if (makeWindowId(candidate->window()) == *candidateId) {
        const bool moved =
            runtime()->moveTiledWindow(makeWindowId(t->window()), *candidateId);
        if (!moved) {
          return;
        }
        recalculate();
        return;
      }
    }

    recalculate();
  }

  SP<Layout::ITarget> getNextCandidate(SP<Layout::ITarget> old) override {
    const auto parent = m_parent.lock();
    if (!parent || !old || !old->window() || !runtime()) {
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

    const auto candidateId =
        runtime()->layoutCloseFocusCandidate(makeWindowId(old->window()));
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
      if (makeWindowId(target->window()) == *candidateId) {
        return target;
      }
    }

    return old;
  }
};

std::string makeWindowId(const PHLWINDOW &window) {
  const auto rawId =
      static_cast<WINDOWID>(reinterpret_cast<uintptr_t>(window.get()));
  const auto it = g_windowIds.find(rawId);
  if (it != g_windowIds.end()) {
    return it->second;
  }

  std::ostringstream stream;
  stream << "hypr-window-" << rawId;
  auto id = stream.str();
  g_windowIds.emplace(rawId, id);
  return id;
}

void forgetWindowId(const PHLWINDOW &window) {
  if (!window) {
    return;
  }

  const auto rawId =
      static_cast<WINDOWID>(reinterpret_cast<uintptr_t>(window.get()));
  g_windowIds.erase(rawId);
}

std::string workspaceName(const PHLWORKSPACE &workspace) {
  if (!workspace) {
    return "1";
  }

  return workspace->getConfigName();
}

std::string monitorId(const PHLMONITOR &monitor) {
  if (!monitor) {
    return "hyprland";
  }

  return monitor->m_name.empty() ? std::to_string(monitor->m_id)
                                 : monitor->m_name;
}

WindowSyncPayload makeUpsertWindowRequest(const PHLWINDOW &window) {
  const auto windowId = makeWindowId(window);
  const auto workspaceId = workspaceName(window->m_workspace);
  const auto outputId = monitorId(window->m_monitor.lock());

  auto payload = WindowSyncPayload{
      .windowId = windowId,
      .workspaceId = workspaceId,
      .outputId = outputId,
      .ffi =
          {
              .window_id = nullptr,
              .workspace_id = nullptr,
              .output_id = nullptr,
              .is_xwayland = window->m_isX11,
              .mapped = window->m_isMapped,
              .title = nullptr,
              .app_id = nullptr,
              .class_name = nullptr,
              .instance = nullptr,
              .role = nullptr,
              .window_type = nullptr,
              .urgent = window->m_isUrgent,
              .floating = window->m_isFloating,
              .fullscreen = window->isFullscreen(),
          },
  };

  payload.ffi.window_id = payload.windowId.c_str();
  payload.ffi.workspace_id =
      payload.workspaceId.empty() ? nullptr : payload.workspaceId.c_str();
  payload.ffi.output_id =
      payload.outputId.empty() ? nullptr : payload.outputId.c_str();
  payload.ffi.title =
      window->m_title.empty() ? nullptr : window->m_title.c_str();
  payload.ffi.app_id =
      window->m_class.empty() ? nullptr : window->m_class.c_str();
  payload.ffi.class_name =
      window->m_class.empty() ? nullptr : window->m_class.c_str();
  payload.ffi.instance =
      window->m_initialClass.empty() ? nullptr : window->m_initialClass.c_str();
  return payload;
}

void logSyncResponse(const std::string &response) {
  if (!runtime()) {
    return;
  }

  logJson("sync", response);
}

void syncMonitor(const PHLMONITOR &monitor) {
  if (!monitor) {
    return;
  }

  auto payload = OutputSyncPayload{
      .outputId = monitorId(monitor),
      .name = monitor->m_name.empty() ? monitorId(monitor) : monitor->m_name,
      .ffi =
          {
              .output_id = nullptr,
              .name = nullptr,
              .logical_width =
                  static_cast<int>(monitor->m_size.x) > 0
                      ? static_cast<unsigned int>(monitor->m_size.x)
                      : 1920U,
              .logical_height =
                  static_cast<int>(monitor->m_size.y) > 0
                      ? static_cast<unsigned int>(monitor->m_size.y)
                      : 1080U,
          },
  };

  payload.ffi.output_id = payload.outputId.c_str();
  payload.ffi.name = payload.name.c_str();
  logSyncResponse(runtime()->upsertOutput(payload.ffi));
}

void syncWorkspace(const PHLWORKSPACE &workspace, const PHLMONITOR &monitor) {
  if (!workspace || !runtime()) {
    return;
  }

  logSyncResponse(runtime()->activateWorkspace(workspaceName(workspace),
                                               monitorId(monitor)));
  syncWorkspaceLayoutSpace(workspace);
}

void syncWorkspaceLayoutSpace(const PHLWORKSPACE &workspace) {
  if (!workspace || !workspace->m_space || !runtime()) {
    return;
  }

  const auto monitor = workspace->m_monitor.lock();
  const auto workArea = workspace->m_space->workArea(false);
  auto payload = WorkspaceLayoutSpaceSyncPayload{
      .workspaceId = workspaceName(workspace),
      .outputId = monitorId(monitor),
      .ffi =
          {
              .workspace_id = nullptr,
              .output_id = nullptr,
              .x = static_cast<int>(workArea.x),
              .y = static_cast<int>(workArea.y),
              .width =
                  workArea.w > 0 ? static_cast<unsigned int>(workArea.w) : 0U,
              .height =
                  workArea.h > 0 ? static_cast<unsigned int>(workArea.h) : 0U,
          },
  };

  payload.ffi.workspace_id = payload.workspaceId.c_str();
  payload.ffi.output_id =
      payload.outputId.empty() ? nullptr : payload.outputId.c_str();
  logSyncResponse(runtime()->setWorkspaceLayoutSpace(payload.ffi));
}

void syncWindow(const PHLWINDOW &window) {
  if (!window || !runtime()) {
    return;
  }

  // Hyprland may emit open/update events for provisional window objects before
  // they are fully mapped. Keeping those placeholders in the runtime pollutes
  // Spider's window set and causes inconsistent placement while a new tiled
  // target is opening.
  if (!window->m_isMapped) {
    removeWindow(window);
    return;
  }

  const auto payload = makeUpsertWindowRequest(window);
  logSyncResponse(runtime()->upsertWindow(payload.ffi));
}

void syncWorkspaceWindows(const PHLWORKSPACE &workspace) {
  if (!workspace || !workspace->m_space || !runtime()) {
    return;
  }

  for (const auto &weakTarget : workspace->m_space->targets()) {
    const auto target = weakTarget.lock();
    if (!target || target->floating() || !target->window()) {
      continue;
    }

    if (target->window()->m_workspace != workspace ||
        !target->window()->m_isMapped) {
      continue;
    }

    syncWindow(target->window());
  }
}

void recalculateWorkspace(const PHLWORKSPACE &workspace) {
  if (!workspace || !workspace->m_space) {
    return;
  }

  workspace->m_space->recheckWorkArea();
  syncWorkspaceLayoutSpace(workspace);
  workspace->m_space->recalculate();
  workspace->updateWindows();
  workspace->forceReportSizesToWindows();

  const auto monitor = workspace->m_monitor.lock();
  if (monitor && g_layoutManager) {
    g_layoutManager->recalculateMonitor(monitor);
  }
}

void applyPlacementForWorkspace(const PHLWORKSPACE &workspace) {
  if (!workspace || !workspace->m_space || !runtime()) {
    return;
  }

  const auto monitor = workspace->m_monitor.lock();
  if (!monitor || monitor->m_activeWorkspace != workspace) {
    return;
  }

  const auto placement =
      runtime()->layoutPlacementForWorkspace(workspaceName(workspace));
  const auto byWindowId = geometryMapFromPlacement(placement);
  hypreact_runtime_free_placement_result(placement);

  for (const auto &window : g_pCompositor->m_windows) {
    if (!window || !window->m_isMapped || window->m_isFloating ||
        !window->m_target) {
      continue;
    }

    if (window->m_workspace != workspace) {
      continue;
    }

    const auto it = byWindowId.find(makeWindowId(window));
    if (it == byWindowId.end() || it->second.w <= 0 || it->second.h <= 0) {
      continue;
    }

    window->m_target->setPositionGlobal(
        offsetPlacementToWorkspace(it->second, workspace));
    window->m_target->warpPositionSize();
  }

  workspace->updateWindows();
  workspace->forceReportSizesToWindows();
}

void queueWorkspaceRecalculate(const PHLWORKSPACE &workspace) {
  if (!workspace) {
    return;
  }

  if (isWorkspaceInRecentResizeWindow(workspace)) {
    return;
  }

  for (auto &pending : g_pendingWorkspaceRecalculations) {
    if (pending.workspace.get() == workspace.get()) {
      pending.remainingTicks = std::max(pending.remainingTicks, 4);
      return;
    }
  }

  g_pendingWorkspaceRecalculations.push_back(PendingWorkspaceRecalculation{
      .workspace = workspace,
      .remainingTicks = 4,
  });

  g_pendingWorkspaceLayoutRefreshTicks =
      std::max(g_pendingWorkspaceLayoutRefreshTicks, 4);
}

void flushPendingWorkspaceRecalculations() {
  std::vector<RecentWorkspaceResize> stillRecentResizes;
  stillRecentResizes.reserve(g_recentWorkspaceResizes.size());
  for (auto recent : g_recentWorkspaceResizes) {
    if (recent.workspace && !recent.workspace->inert() &&
        --recent.remainingTicks > 0) {
      stillRecentResizes.push_back(std::move(recent));
    }
  }
  g_recentWorkspaceResizes = std::move(stillRecentResizes);

  if (g_pendingWorkspaceLayoutRefreshTicks > 0) {
    Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
    --g_pendingWorkspaceLayoutRefreshTicks;
  }

  std::vector<PendingWorkspaceRecalculation> stillPending;
  stillPending.reserve(g_pendingWorkspaceRecalculations.size());

  for (auto pending : g_pendingWorkspaceRecalculations) {
    if (pending.workspace && !pending.workspace->inert()) {
      const auto monitor = pending.workspace->m_monitor.lock();
      if (!monitor || monitor->m_activeWorkspace != pending.workspace) {
        stillPending.push_back(std::move(pending));
        continue;
      }

      recalculateWorkspace(pending.workspace);

      if (--pending.remainingTicks > 0) {
        stillPending.push_back(std::move(pending));
      }
    }
  }

  g_pendingWorkspaceRecalculations = std::move(stillPending);
}

void markRecentWorkspaceResize(const PHLWORKSPACE &workspace) {
  if (!workspace) {
    return;
  }

  for (auto &recent : g_recentWorkspaceResizes) {
    if (recent.workspace.get() == workspace.get()) {
      recent.remainingTicks = std::max(recent.remainingTicks, 3);
      return;
    }
  }

  g_recentWorkspaceResizes.push_back(RecentWorkspaceResize{
      .workspace = workspace,
      .remainingTicks = 3,
  });
}

bool isWorkspaceInRecentResizeWindow(const PHLWORKSPACE &workspace) {
  return workspace &&
         std::any_of(g_recentWorkspaceResizes.begin(),
                     g_recentWorkspaceResizes.end(),
                     [&](const RecentWorkspaceResize &recent) {
                       return recent.workspace.get() == workspace.get();
                     });
}

void syncActiveRuntimeState() {
  if (!runtime()) {
    return;
  }

  for (const auto &monitor : g_pCompositor->m_monitors) {
    if (monitor && monitor->m_activeWorkspace) {
      syncWorkspace(monitor->m_activeWorkspace, monitor);
    }
  }

  if (const auto focus = Desktop::focusState()) {
    syncFocusedWindow(focus->window());
  }
}

void recalculateWindowWorkspace(const PHLWINDOW &window) {
  if (!window) {
    return;
  }

  queueWorkspaceRecalculate(window->m_workspace);
}

void syncFocusedWindow(const PHLWINDOW &window) {
  if (!runtime()) {
    return;
  }

  logSyncResponse(runtime()->focusWindow(
      window ? std::optional<std::string>(makeWindowId(window))
             : std::nullopt));
}

void markWindowClosing(const PHLWINDOW &window, bool closing) {
  if (!window || !runtime()) {
    return;
  }

  logSyncResponse(runtime()->setWindowClosing(makeWindowId(window), closing));
  queueWorkspaceRecalculate(window->m_workspace);
}

void removeWindow(const PHLWINDOW &window) {
  if (!window || !runtime()) {
    return;
  }

  const auto workspace = window->m_workspace;
  const auto response = runtime()->removeWindow(makeWindowId(window));
  logSyncResponse(response);
  if (workspace) {
    queueWorkspaceRecalculate(workspace);
  }
  if (const auto parsed = parseJson(response)) {
    const auto focusedWindowId =
        (*parsed)["data"]["focusedWindowId"].asString();
    (void)focusedWindowId;
  }
  forgetWindowId(window);
}

void resyncAll() {
  if (!runtime()) {
    return;
  }

  g_windowIds.clear();
  logSyncResponse(runtime()->resetState());

  for (const auto &monitor : g_pCompositor->m_monitors) {
    syncMonitor(monitor);
    if (monitor && monitor->m_activeWorkspace) {
      syncWorkspace(monitor->m_activeWorkspace, monitor);
    }
  }

  for (const auto &window : g_pCompositor->m_windows) {
    if (window && window->m_isMapped) {
      syncWindow(window);
    }
  }

  if (const auto focus = Desktop::focusState()) {
    syncFocusedWindow(focus->window());
  }

  for (const auto &monitor : g_pCompositor->m_monitors) {
    if (monitor && monitor->m_activeWorkspace) {
      queueWorkspaceRecalculate(monitor->m_activeWorkspace);
    }
  }
}

void refreshWorkspaceAlgorithms() {
  if (!g_registeredHypreactAlgo || !layoutRuntimeLoaded()) {
    return;
  }

  Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
}

void registerHypreactAlgorithm() {
  if (g_registeredHypreactAlgo) {
    return;
  }

  g_registeredHypreactAlgo = HyprlandAPI::addTiledAlgo(
      PHANDLE, "hypreact", &typeid(CHypreactAlgorithm),
      [] { return makeUnique<CHypreactAlgorithm>(); });

  if (g_registeredHypreactAlgo) {
    std::cout << "[hypreact] registered tiled algorithm: hypreact" << std::endl;
  } else {
    std::cerr << "[hypreact] failed to register tiled algorithm: hypreact"
              << std::endl;
  }
}

void unregisterHypreactAlgorithm() {
  if (!g_registeredHypreactAlgo) {
    return;
  }

  if (!HyprlandAPI::removeAlgo(PHANDLE, "hypreact")) {
    std::cerr << "[hypreact] failed to unregister tiled algorithm: hypreact"
              << std::endl;
    return;
  }

  std::cout << "[hypreact] unregistered tiled algorithm: hypreact" << std::endl;
  g_registeredHypreactAlgo = false;
}

SDispatchResult callDispatcher(const std::string &name,
                               const std::string &arg) {
  const auto it = g_pKeybindManager->m_dispatchers.find(name);
  if (it == g_pKeybindManager->m_dispatchers.end()) {
    return {.passEvent = false,
            .success = false,
            .error = "unknown dispatcher: " + name};
  }

  return it->second(arg);
}

SDispatchResult applyActions(const HypreactActionResult &response) {
  if (response.error != nullptr) {
    return {.passEvent = false,
            .success = false,
            .error = std::string(response.error)};
  }

  for (size_t i = 0; i < response.action_count; ++i) {
    const auto &action = response.actions[i];
    SDispatchResult result;

    switch (action.kind) {
    case HYPREACT_ACTION_SPAWN_COMMAND:
      result = callDispatcher("exec",
                              action.string_value ? action.string_value : "");
      break;
    case HYPREACT_ACTION_RELOAD_CONFIG:
      HyprlandAPI::reloadConfig();
      break;
    case HYPREACT_ACTION_SET_LAYOUT:
      result = callDispatcher("layoutmsg",
                              "layout " + std::string(action.string_value
                                                          ? action.string_value
                                                          : ""));
      break;
    case HYPREACT_ACTION_CYCLE_LAYOUT:
      result =
          callDispatcher("layoutmsg", action.has_cycle_direction &&
                                              action.cycle_direction ==
                                                  HYPREACT_LAYOUT_CYCLE_PREVIOUS
                                          ? "cycleprev"
                                          : "cyclenext");
      break;
    case HYPREACT_ACTION_ACTIVATE_WORKSPACE:
      result = callDispatcher("workspace",
                              action.string_value ? action.string_value : "");
      break;
    case HYPREACT_ACTION_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE:
      result =
          callDispatcher("movetoworkspace", std::to_string(action.workspace));
      break;
    case HYPREACT_ACTION_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE:
      result = callDispatcher("movetoworkspacesilent",
                              std::to_string(action.workspace));
      break;
    case HYPREACT_ACTION_TOGGLE_FLOATING:
      result = callDispatcher("togglefloating", "");
      break;
    case HYPREACT_ACTION_TOGGLE_FULLSCREEN:
      result = callDispatcher("fullscreen", "1");
      break;
    case HYPREACT_ACTION_FOCUS_WINDOW:
      result = callDispatcher("focuswindow",
                              "address:" + std::string(action.string_value
                                                           ? action.string_value
                                                           : ""));
      break;
    case HYPREACT_ACTION_FOCUS_DIRECTION:
      result = callDispatcher("movefocus", fromFfiDirection(action.direction));
      break;
    case HYPREACT_ACTION_FOCUS_NEXT_WINDOW:
      result = callDispatcher("cyclenext", "");
      break;
    case HYPREACT_ACTION_FOCUS_PREVIOUS_WINDOW:
      result = callDispatcher("cyclenext", "prev");
      break;
    case HYPREACT_ACTION_SWAP_DIRECTION:
      result = callDispatcher("swapwindow", fromFfiDirection(action.direction));
      break;
    case HYPREACT_ACTION_MOVE_DIRECTION:
      result = callDispatcher("moveactive", fromFfiDirection(action.direction));
      break;
    case HYPREACT_ACTION_RESIZE_DIRECTION:
      result =
          callDispatcher("resizeactive", fromFfiDirection(action.direction));
      break;
    case HYPREACT_ACTION_CLOSE_FOCUSED_WINDOW:
      result = callDispatcher("killactive", "");
      break;
    }

    if (!result.success) {
      return result;
    }
  }

  return {};
}

void registerHooks() {
  auto &events = Event::bus()->m_events;

  g_listeners.push_back(events.tick.listen([] {
    syncActiveRuntimeState();
    flushPendingWorkspaceRecalculations();
  }));

  g_listeners.push_back(events.monitor.added.listen([](PHLMONITOR monitor) {
    syncMonitor(monitor);
    if (monitor && monitor->m_activeWorkspace) {
      syncWorkspace(monitor->m_activeWorkspace, monitor);
      queueWorkspaceRecalculate(monitor->m_activeWorkspace);
    }
  }));

  g_listeners.push_back(events.monitor.removed.listen([](PHLMONITOR monitor) {
    if (runtime() && monitor) {
      logSyncResponse(runtime()->removeOutput(monitorId(monitor)));
    }
  }));

  g_listeners.push_back(events.monitor.focused.listen([](PHLMONITOR monitor) {
    if (monitor && monitor->m_activeWorkspace) {
      syncWorkspace(monitor->m_activeWorkspace, monitor);
      queueWorkspaceRecalculate(monitor->m_activeWorkspace);
    }
  }));

  g_listeners.push_back(
      events.window.open.listen([](PHLWINDOW window) { syncWindow(window); }));

  g_listeners.push_back(events.window.close.listen(
      [](PHLWINDOW window) { markWindowClosing(window, true); }));

  g_listeners.push_back(events.window.destroy.listen(
      [](PHLWINDOW window) { removeWindow(window); }));

  g_listeners.push_back(
      events.window.active.listen([](PHLWINDOW window, Desktop::eFocusReason) {
        if (window) {
          syncWindow(window);
          syncWorkspace(window->m_workspace, window->m_monitor.lock());
          recalculateWindowWorkspace(window);
        }
        syncFocusedWindow(window);
      }));

  g_listeners.push_back(events.window.title.listen([](PHLWINDOW window) {
    syncWindow(window);
    recalculateWindowWorkspace(window);
  }));

  g_listeners.push_back(events.window.class_.listen([](PHLWINDOW window) {
    syncWindow(window);
    recalculateWindowWorkspace(window);
  }));

  g_listeners.push_back(events.window.updateRules.listen([](PHLWINDOW window) {
    syncWindow(window);
    recalculateWindowWorkspace(window);
  }));

  g_listeners.push_back(events.window.fullscreen.listen(
      [](PHLWINDOW window) { syncWindow(window); }));

  g_listeners.push_back(events.window.urgent.listen(
      [](PHLWINDOW window) { syncWindow(window); }));

  g_listeners.push_back(
      events.window.pin.listen([](PHLWINDOW window) { syncWindow(window); }));

  g_listeners.push_back(events.window.moveToWorkspace.listen(
      [](PHLWINDOW window, PHLWORKSPACE workspace) {
        syncWindow(window);
        syncWorkspace(workspace, window ? window->m_monitor.lock() : nullptr);
        queueWorkspaceRecalculate(workspace);
      }));

  g_listeners.push_back(
      events.workspace.active.listen([](PHLWORKSPACE workspace) {
        syncWorkspace(workspace,
                      workspace ? workspace->m_monitor.lock() : nullptr);
        queueWorkspaceRecalculate(workspace);
      }));

  g_listeners.push_back(events.config.reloaded.listen([] {
    loadLayoutRuntimeConfig();
    if (layoutRuntimeLoaded()) {
      registerHypreactAlgorithm();
      refreshWorkspaceAlgorithms();
    }
    resyncAll();
    flushPendingWorkspaceRecalculations();
  }));
}

void appendWorkspaceNames(Json::Value &target, char **workspaceNames,
                          size_t workspaceNameCount) {
  for (size_t i = 0; i < workspaceNameCount; ++i) {
    if (workspaceNames[i] != nullptr) {
      target.append(workspaceNames[i]);
    }
  }
}

void appendPlacement(Json::Value &target,
                     const HypreactPlacementResult &placement) {
  for (size_t i = 0; i < placement.geometry_count; ++i) {
    Json::Value geometry;
    if (placement.geometries[i].window_id != nullptr) {
      geometry["windowId"] = placement.geometries[i].window_id;
    }
    geometry["x"] = placement.geometries[i].x;
    geometry["y"] = placement.geometries[i].y;
    geometry["width"] = placement.geometries[i].width;
    geometry["height"] = placement.geometries[i].height;
    target.append(geometry);
  }
}

void appendLayoutStatus(Json::Value &target,
                        const HypreactLayoutStatusResult &layout) {
  target["loaded"] = layout.loaded;
  if (layout.config_path != nullptr) {
    target["configPath"] = layout.config_path;
  }
  if (layout.selected_layout_name != nullptr) {
    target["selectedLayoutName"] = layout.selected_layout_name;
  }
  if (layout.error != nullptr) {
    target["error"] = layout.error;
  }
  if (layout.diagnostics_json != nullptr) {
    if (const auto diagnostics = parseJson(layout.diagnostics_json)) {
      target["diagnostics"] = *diagnostics;
    }
  }
  appendWorkspaceNames(target["workspaceNames"], layout.workspace_names,
                       layout.workspace_name_count);
}

void appendRuntimeState(Json::Value &target, const HypreactStateResult &state) {
  if (state.current_workspace_id != nullptr) {
    target["currentWorkspaceId"] = state.current_workspace_id;
  }
  if (state.current_output_id != nullptr) {
    target["currentOutputId"] = state.current_output_id;
  }
  if (state.focused_window_id != nullptr) {
    target["focusedWindowId"] = state.focused_window_id;
  }
  appendWorkspaceNames(target["workspaceNames"], state.workspace_names,
                       state.workspace_name_count);
}

std::string queryRuntime(eHyprCtlOutputFormat, std::string arg) {
  if (!runtime()) {
    return R"({"ok":false,"error":"runtime not initialized"})";
  }

  auto command = trim(arg);
  if (command == "hypreact") {
    command.clear();
  } else if (command.rfind("hypreact ", 0) == 0) {
    command = trim(command.substr(std::string("hypreact ").size()));
  }

  if (command == "resync") {
    resyncAll();
    return R"({"ok":true,"data":{"message":"resynced"}})";
  }

  if (command == "layouts") {
    loadLayoutRuntimeConfig();
    const auto layout = runtime()->layoutStatusResult();
    Json::Value response;
    response["ok"] = true;
    appendLayoutStatus(response["data"], layout);
    hypreact_runtime_free_layout_status_result(layout);
    return stringify(response);
  }

  if (command == "debug-layout") {
    loadLayoutRuntimeConfig();
    const auto layout = runtime()->layoutStatusResult();
    Json::Value response;
    response["ok"] = true;
    appendLayoutStatus(response["data"], layout);

    const auto placement = runtime()->layoutPlacement();
    appendPlacement(response["data"]["placement"], placement);
    hypreact_runtime_free_placement_result(placement);
    hypreact_runtime_free_layout_status_result(layout);
    return stringify(response);
  }

  if (command.rfind("debug-layout-workspace ", 0) == 0) {
    loadLayoutRuntimeConfig();
    const auto workspaceId =
        trim(command.substr(std::string("debug-layout-workspace ").size()));
    Json::Value response;
    response["ok"] = true;
    response["data"]["workspaceId"] = workspaceId;

    const auto placement = runtime()->layoutPlacementForWorkspace(workspaceId);
    appendPlacement(response["data"]["placement"], placement);

    hypreact_runtime_free_placement_result(placement);
    return stringify(response);
  }

  if (command == "reload-layouts") {
    loadLayoutRuntimeConfig();
    const auto result = runtime()->reloadLayoutConfig();
    Json::Value response;
    response["ok"] = result.error == nullptr;
    response["data"]["changed"] = result.changed;
    if (result.error != nullptr) {
      response["error"] = result.error;
    }
    hypreact_runtime_free_status_result(result);
    return stringify(response);
  }

  Json::Value response;
  response["ok"] = true;

  const auto state = runtime()->stateResult();
  appendRuntimeState(response["data"]["runtime"], state);
  hypreact_runtime_free_state_result(state);

  loadLayoutRuntimeConfig();
  const auto layout = runtime()->layoutStatusResult();
  appendLayoutStatus(response["data"]["layouts"], layout);
  hypreact_runtime_free_layout_status_result(layout);

  return stringify(response);
}

} // namespace

#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wreturn-type-c-linkage"
#endif

extern "C" EXPORT std::string pluginAPIVersion() {
  return HYPRLAND_API_VERSION;
}

extern "C" EXPORT PLUGIN_DESCRIPTION_INFO pluginInit(HANDLE handle) {
  PHANDLE = handle;
  setPluginHandle(handle);

  HyprlandAPI::addConfigValue(PHANDLE, "plugin:hypreact:config_path",
                              Hyprlang::CConfigValue(""));
  setConfigPathValue(
      HyprlandAPI::getConfigValue(PHANDLE, "plugin:hypreact:config_path"));

  createRuntime();
  resyncAll();
  loadLayoutRuntimeConfig();
  if (layoutRuntimeLoaded()) {
    registerHypreactAlgorithm();
    refreshWorkspaceAlgorithms();
  }
  registerHypreactDispatchers();
  registerHooks();

  g_queryCommand =
      HyprlandAPI::registerHyprCtlCommand(PHANDLE, SHyprCtlCommand{
                                                       .name = "hypreact",
                                                       .exact = false,
                                                       .fn = queryRuntime,
                                                   });

  if (!g_queryCommand) {
    std::cerr << "[hypreact] failed to register hyprctl command: hypreact"
              << std::endl;
  } else {
    std::cout << "[hypreact] registered hyprctl command: hypreact" << std::endl;
  }

  HyprlandAPI::addNotificationV2(PHANDLE,
                                 {
                                     {"text", std::string{"hypreact loaded"}},
                                     {"time", static_cast<uint64_t>(3000)},
                                     {"icon", ICON_INFO},
                                 });

  return {
      .name = "hypreact",
      .description = "Hyprland plugin bridge for hypreact",
      .author = "OpenCode",
      .version = "0.1.0",
  };
}

extern "C" EXPORT void pluginExit() {
  if (PHANDLE != nullptr) {
    if (g_queryCommand) {
      if (!HyprlandAPI::unregisterHyprCtlCommand(PHANDLE, g_queryCommand)) {
        std::cerr << "[hypreact] failed to unregister hyprctl command: hypreact"
                  << std::endl;
      } else {
        std::cout << "[hypreact] unregistered hyprctl command: hypreact"
                  << std::endl;
      }
      g_queryCommand.reset();
    }
  }

  g_listeners.clear();
  g_pendingWorkspaceRecalculations.clear();
  g_pendingWorkspaceLayoutRefreshTicks = 0;
  g_windowIds.clear();
  clearConfigPathValue();
  unregisterHypreactAlgorithm();
  destroyRuntime();
  clearPluginHandle();
  PHANDLE = nullptr;
}

#ifdef __clang__
#pragma clang diagnostic pop
#endif
