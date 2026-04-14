#include "hypreact_plugin_sync.hpp"

#include <algorithm>
#include <iostream>
#include <optional>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

#include "hypreact_plugin_algorithm.hpp"
#include "hypreact_hypr_ffi.h"
#include "hypreact_plugin_runtime.hpp"

#include "src/Compositor.hpp"
#include "src/desktop/state/FocusState.hpp"
#include "src/layout/space/Space.hpp"

namespace hypreact_plugin {
namespace {

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

std::unordered_map<WINDOWID, std::string> g_windowIds;
std::vector<PendingWorkspaceRecalculation> g_pendingWorkspaceRecalculations;
std::vector<RecentWorkspaceResize> g_recentWorkspaceResizes;
int g_pendingWorkspaceLayoutRefreshTicks = 0;

void forgetWindowId(const PHLWINDOW &window) {
  if (!window) {
    return;
  }

  const auto rawId =
      static_cast<WINDOWID>(reinterpret_cast<uintptr_t>(window.get()));
  g_windowIds.erase(rawId);
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
  payload.ffi.title = window->m_title.empty() ? nullptr : window->m_title.c_str();
  payload.ffi.app_id = window->m_class.empty() ? nullptr : window->m_class.c_str();
  payload.ffi.class_name =
      window->m_class.empty() ? nullptr : window->m_class.c_str();
  payload.ffi.instance =
      window->m_initialClass.empty() ? nullptr : window->m_initialClass.c_str();
  return payload;
}

void logStatusResult(const char *label, const HypreactStatusResult &result) {
  if (result.error != nullptr) {
    std::cerr << "[hypreact] " << label << " failed: " << result.error
              << std::endl;
  }
}

void logAndFreeStatusResult(const char *label, HypreactStatusResult result) {
  logStatusResult(label, result);
  hypreact_runtime_free_status_result(result);
}

} // namespace

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
  logAndFreeStatusResult("sync-output", runtime()->upsertOutput(payload.ffi));
}

void syncWorkspace(const PHLWORKSPACE &workspace, const PHLMONITOR &monitor) {
  if (!workspace || !runtime()) {
    return;
  }

  logAndFreeStatusResult("sync-workspace",
                         runtime()->activateWorkspace(workspaceName(workspace),
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
              .width = workArea.w > 0 ? static_cast<unsigned int>(workArea.w) : 0U,
              .height =
                  workArea.h > 0 ? static_cast<unsigned int>(workArea.h) : 0U,
          },
  };

  payload.ffi.workspace_id = payload.workspaceId.c_str();
  payload.ffi.output_id =
      payload.outputId.empty() ? nullptr : payload.outputId.c_str();
  logAndFreeStatusResult("sync-layout-space",
                         runtime()->setWorkspaceLayoutSpace(payload.ffi));
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
  logAndFreeStatusResult("sync-window", runtime()->upsertWindow(payload.ffi));
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
    refreshWorkspaceAlgorithms();
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

  logAndFreeStatusResult(
      "focus-window",
      runtime()->focusWindow(window ? std::optional<std::string>(makeWindowId(window))
                                    : std::nullopt));
}

void markWindowClosing(const PHLWINDOW &window, bool closing) {
  if (!window || !runtime()) {
    return;
  }

  logAndFreeStatusResult("set-window-closing",
                         runtime()->setWindowClosing(makeWindowId(window),
                                                     closing));
  queueWorkspaceRecalculate(window->m_workspace);
}

void removeWindow(const PHLWINDOW &window) {
  if (!window || !runtime()) {
    return;
  }

  const auto workspace = window->m_workspace;
  const auto result = runtime()->removeWindow(makeWindowId(window));
  logAndFreeStatusResult("remove-window", result);
  if (workspace) {
    queueWorkspaceRecalculate(workspace);
  }
  forgetWindowId(window);
}

void removeOutput(const PHLMONITOR &monitor) {
  if (!runtime() || !monitor) {
    return;
  }

  logAndFreeStatusResult("remove-output",
                         runtime()->removeOutput(monitorId(monitor)));
}

void resyncAll() {
  if (!runtime()) {
    return;
  }

  g_windowIds.clear();
  logAndFreeStatusResult("reset-state", runtime()->resetState());

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

void clearSyncState() {
  g_pendingWorkspaceRecalculations.clear();
  g_recentWorkspaceResizes.clear();
  g_pendingWorkspaceLayoutRefreshTicks = 0;
  g_windowIds.clear();
}

} // namespace hypreact_plugin
