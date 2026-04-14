#include "hypreact_plugin_hooks.hpp"

#include <vector>

#include "hypreact_plugin_sync.hpp"

#include "src/SharedDefs.hpp"
#include "src/desktop/state/FocusState.hpp"
#include "src/event/EventBus.hpp"

namespace hypreact_plugin {
namespace {

std::vector<CHyprSignalListener> g_listeners;
const HookCallbacks *g_hookCallbacks = nullptr;

} // namespace

void registerHooks(const HookCallbacks &callbacks) {
  g_hookCallbacks = &callbacks;

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
    removeOutput(monitor);
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

  g_listeners.push_back(
      events.window.destroy.listen([](PHLWINDOW window) { removeWindow(window); }));

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
    g_hookCallbacks->loadLayoutRuntimeConfig();
    if (g_hookCallbacks->layoutRuntimeLoaded()) {
      g_hookCallbacks->registerHypreactAlgorithm();
      g_hookCallbacks->refreshWorkspaceAlgorithms();
    }
    resyncAll();
    flushPendingWorkspaceRecalculations();
  }));
}

void clearHooks() {
  g_listeners.clear();
  g_hookCallbacks = nullptr;
}

} // namespace hypreact_plugin
