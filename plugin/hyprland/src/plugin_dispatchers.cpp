#include "tilescript_hypr_plugin_dispatchers.hpp"

#include <optional>
#include <sstream>
#include <string>

#include "tilescript_hypr_plugin_sync.hpp"
#include "tilescript_hypr_plugin_runtime.hpp"

#include "src/Compositor.hpp"
#include "src/desktop/state/FocusState.hpp"
#include "src/desktop/view/Window.hpp"
#include "src/layout/space/Space.hpp"
#include "src/plugins/PluginAPI.hpp"

namespace tilescript_plugin {
namespace {

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

PHLWINDOW windowFromTilescriptId(const std::string &windowId,
                               const DispatcherCallbacks &callbacks) {
  for (const auto &window : g_pCompositor->m_windows) {
    if (!window || !window->m_isMapped) {
      continue;
    }

    if (callbacks.makeWindowId(window) == windowId) {
      return window;
    }
  }

  return nullptr;
}

SDispatchResult
tilescriptMoveFocusDispatcher(std::string arg,
                            const DispatcherCallbacks &callbacks) {
  if (!runtime()) {
    return {.success = false, .error = "runtime not initialized"};
  }

  const auto direction = normalizeDirection(arg);
  if (!direction.has_value()) {
    return {.success = false, .error = "invalid direction"};
  }

  const auto focusedWindow = Desktop::focusState()->window();
  if (focusedWindow) {
    callbacks.syncWindow(focusedWindow);
    callbacks.syncWorkspace(focusedWindow->m_workspace,
                            focusedWindow->m_monitor.lock());
    callbacks.syncFocusedWindow(focusedWindow);
  }

  const auto target = runtime()->layoutFocusCandidate(*direction);
  if (!target.has_value()) {
    return {};
  }

  const auto targetWindow = windowFromTilescriptId(*target, callbacks);
  if (!targetWindow) {
    return {.success = false, .error = "target window not found"};
  }

  std::ostringstream address;
  address << "address:0x" << std::hex
          << reinterpret_cast<uintptr_t>(targetWindow.get());
  return callbacks.callDispatcher("focuswindow", address.str());
}

SDispatchResult
tilescriptMoveWindowDispatcher(std::string arg,
                             const DispatcherCallbacks &callbacks) {
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
    return callbacks.callDispatcher("movewindow", direction->substr(0, 1));
  }

  callbacks.syncWindow(focusedWindow);
  callbacks.syncWorkspace(focusedWindow->m_workspace,
                          focusedWindow->m_monitor.lock());
  callbacks.syncWorkspaceWindows(focusedWindow->m_workspace);
  callbacks.syncFocusedWindow(focusedWindow);

  const auto candidateId = runtime()->layoutSwapCandidate(*direction);
  if (!candidateId.has_value()) {
    return {};
  }

  const auto candidateWindow = windowFromTilescriptId(*candidateId, callbacks);
  if (!candidateWindow) {
    return {.success = false, .error = "target window not found"};
  }

  const auto focusedWindowId = callbacks.makeWindowId(focusedWindow);
  if (!runtime()->moveTiledWindow(focusedWindowId, *candidateId)) {
    return {.success = false, .error = "failed to move tiled window"};
  }

  const auto workspace = focusedWindow->m_workspace;
  if (workspace) {
    callbacks.recalculateWorkspace(workspace);
  }

  return {};
}

SDispatchResult
tilescriptResizeWindowDispatcher(std::string arg,
                               const DispatcherCallbacks &callbacks) {
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
    return callbacks.callDispatcher("resizeactive", direction->substr(0, 1) +
                                                        std::string(" 40"));
  }

  callbacks.syncWindow(focusedWindow);
  callbacks.syncWorkspace(focusedWindow->m_workspace,
                          focusedWindow->m_monitor.lock());
  callbacks.syncWorkspaceWindows(focusedWindow->m_workspace);
  callbacks.syncFocusedWindow(focusedWindow);

  if (!runtime()->resizeDirection(*direction)) {
    return {.success = false, .error = "no resize candidate"};
  }

  const auto workspace = focusedWindow->m_workspace;
  if (workspace) {
    callbacks.applyPlacementForWorkspace(workspace);
    callbacks.markRecentWorkspaceResize(workspace);
    if (workspace->m_space) {
      workspace->m_space->recalculate();
    }
    callbacks.queueWorkspaceRecalculate(workspace);
  }

  return {};
}

} // namespace

void registerTilescriptDispatchers(HANDLE pluginHandle,
                                 const DispatcherCallbacks &callbacks) {
  HyprlandAPI::addDispatcherV2(pluginHandle, "tilescript:movefocus",
                               [callbacks](std::string arg) {
                                 return tilescriptMoveFocusDispatcher(
                                     std::move(arg), callbacks);
                               });
  HyprlandAPI::addDispatcherV2(pluginHandle, "tilescript:movewindow",
                               [callbacks](std::string arg) {
                                 return tilescriptMoveWindowDispatcher(
                                     std::move(arg), callbacks);
                               });
  HyprlandAPI::addDispatcherV2(pluginHandle, "tilescript:resizewindow",
                               [callbacks](std::string arg) {
                                 return tilescriptResizeWindowDispatcher(
                                     std::move(arg), callbacks);
                               });
}

} // namespace tilescript_plugin
