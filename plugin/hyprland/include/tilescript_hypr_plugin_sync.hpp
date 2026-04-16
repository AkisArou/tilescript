#pragma once

#include <string>

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/desktop/view/Window.hpp"
#include "src/helpers/Monitor.hpp"

namespace tilescript_plugin {

std::string makeWindowId(const PHLWINDOW &window);
std::string workspaceName(const PHLWORKSPACE &workspace);
std::string monitorId(const PHLMONITOR &monitor);

void syncMonitor(const PHLMONITOR &monitor);
void syncWorkspace(const PHLWORKSPACE &workspace, const PHLMONITOR &monitor);
void syncWorkspaceLayoutSpace(const PHLWORKSPACE &workspace);
void syncWindow(const PHLWINDOW &window);
void syncWorkspaceWindows(const PHLWORKSPACE &workspace);
void recalculateWorkspace(const PHLWORKSPACE &workspace);
void applyPlacementForWorkspace(const PHLWORKSPACE &workspace);
void queueWorkspaceRecalculate(const PHLWORKSPACE &workspace);
void flushPendingWorkspaceRecalculations();
void markRecentWorkspaceResize(const PHLWORKSPACE &workspace);
bool isWorkspaceInRecentResizeWindow(const PHLWORKSPACE &workspace);
void syncActiveRuntimeState();
void recalculateWindowWorkspace(const PHLWINDOW &window);
void syncFocusedWindow(const PHLWINDOW &window);
void markWindowClosing(const PHLWINDOW &window, bool closing);
void removeWindow(const PHLWINDOW &window);
void removeOutput(const PHLMONITOR &monitor);
void resyncAll();
void clearSyncState();

} // namespace tilescript_plugin
