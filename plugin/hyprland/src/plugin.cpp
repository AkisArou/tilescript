#include <iostream>
#include <sstream>
#include <string>

#include <json/json.h>

#include "hypreact_plugin_algorithm.hpp"
#include "hypreact_plugin_dispatchers.hpp"
#include "hypreact_plugin_hooks.hpp"
#include "hypreact_plugin_sync.hpp"
#include "hypreact_hypr_ffi.h"
#include "hypreact_plugin_query.hpp"
#include "hypreact_plugin_runtime.hpp"

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/managers/KeybindManager.hpp"
#include "src/plugins/PluginAPI.hpp"

inline HANDLE PHANDLE = nullptr;

namespace {

using hypreact_plugin::clearConfigPathValue;
using hypreact_plugin::clearHooks;
using hypreact_plugin::clearPluginHandle;
using hypreact_plugin::clearSyncState;
using hypreact_plugin::createRuntime;
using hypreact_plugin::destroyRuntime;
using hypreact_plugin::layoutRuntimeLoaded;
using hypreact_plugin::loadLayoutRuntimeConfig;
using hypreact_plugin::makeWindowId;
using hypreact_plugin::markRecentWorkspaceResize;
using hypreact_plugin::applyPlacementForWorkspace;
using hypreact_plugin::queueWorkspaceRecalculate;
using hypreact_plugin::recalculateWorkspace;
using hypreact_plugin::runtime;
using hypreact_plugin::resyncAll;
using hypreact_plugin::setConfigPathValue;
using hypreact_plugin::setPluginHandle;
using hypreact_plugin::syncFocusedWindow;
using hypreact_plugin::syncWindow;
using hypreact_plugin::syncWorkspace;
using hypreact_plugin::syncWorkspaceLayoutSpace;
using hypreact_plugin::syncWorkspaceWindows;
using hypreact_plugin::stringify;
using hypreact_plugin::trim;
using hypreact_plugin::workspaceName;

SP<SHyprCtlCommand> g_queryCommand;

SDispatchResult callDispatcher(const std::string &name, const std::string &arg);
void refreshWorkspaceAlgorithms();

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

void registerHypreactDispatchers() {
  hypreact_plugin::registerHypreactDispatchers(
      PHANDLE,
      hypreact_plugin::DispatcherCallbacks{
          .callDispatcher = callDispatcher,
          .makeWindowId = makeWindowId,
          .syncWorkspace = syncWorkspace,
          .syncWindow = syncWindow,
          .syncWorkspaceWindows = syncWorkspaceWindows,
          .recalculateWorkspace = recalculateWorkspace,
          .syncFocusedWindow = syncFocusedWindow,
          .queueWorkspaceRecalculate = queueWorkspaceRecalculate,
          .applyPlacementForWorkspace = applyPlacementForWorkspace,
          .markRecentWorkspaceResize = markRecentWorkspaceResize,
      });
}

void refreshWorkspaceAlgorithms() {
  hypreact_plugin::refreshWorkspaceAlgorithms();
}

void registerHypreactAlgorithm() {
  hypreact_plugin::registerHypreactAlgorithm(
      PHANDLE,
      hypreact_plugin::AlgorithmCallbacks{
          .makeWindowId = makeWindowId,
          .workspaceName = workspaceName,
      });
}

void unregisterHypreactAlgorithm() {
  hypreact_plugin::unregisterHypreactAlgorithm(PHANDLE);
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

std::string queryRuntime(eHyprCtlOutputFormat format, std::string arg) {
  return hypreact_plugin::queryRuntime(format, std::move(arg), resyncAll);
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
  hypreact_plugin::registerHooks({
      .loadLayoutRuntimeConfig = loadLayoutRuntimeConfig,
      .layoutRuntimeLoaded = layoutRuntimeLoaded,
      .registerHypreactAlgorithm = registerHypreactAlgorithm,
      .refreshWorkspaceAlgorithms = refreshWorkspaceAlgorithms,
  });

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

  clearHooks();
  clearSyncState();
  clearConfigPathValue();
  unregisterHypreactAlgorithm();
  destroyRuntime();
  clearPluginHandle();
  PHANDLE = nullptr;
}

#ifdef __clang__
#pragma clang diagnostic pop
#endif
