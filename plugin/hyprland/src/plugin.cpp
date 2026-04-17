#include <iostream>
#include <stdexcept>
#include <sstream>
#include <string>

#include <json/json.h>

#include "tilescript_hypr_ffi.h"
#include "tilescript_hypr_plugin_algorithm.hpp"
#include "tilescript_hypr_plugin_dispatchers.hpp"
#include "tilescript_hypr_plugin_hooks.hpp"
#include "tilescript_hypr_plugin_query.hpp"
#include "tilescript_hypr_plugin_runtime.hpp"
#include "tilescript_hypr_plugin_sync.hpp"

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/managers/KeybindManager.hpp"
#include "src/plugins/PluginAPI.hpp"

inline HANDLE PHANDLE = nullptr;

namespace {

using tilescript_plugin::applyPlacementForWorkspace;
using tilescript_plugin::clearConfigPathValue;
using tilescript_plugin::clearHooks;
using tilescript_plugin::clearPluginHandle;
using tilescript_plugin::clearSyncState;
using tilescript_plugin::createRuntime;
using tilescript_plugin::destroyRuntime;
using tilescript_plugin::layoutRuntimeLoaded;
using tilescript_plugin::loadLayoutRuntimeConfig;
using tilescript_plugin::makeWindowId;
using tilescript_plugin::markRecentWorkspaceResize;
using tilescript_plugin::drainLayoutRuntimeSourceChanges;
using tilescript_plugin::queueWorkspaceRecalculate;
using tilescript_plugin::recalculateWorkspace;
using tilescript_plugin::resyncAll;
using tilescript_plugin::runtime;
using tilescript_plugin::setConfigPathValue;
using tilescript_plugin::setPluginHandle;
using tilescript_plugin::stringify;
using tilescript_plugin::syncFocusedWindow;
using tilescript_plugin::syncWindow;
using tilescript_plugin::syncWorkspace;
using tilescript_plugin::syncWorkspaceLayoutSpace;
using tilescript_plugin::syncWorkspaceWindows;
using tilescript_plugin::trim;
using tilescript_plugin::workspaceName;

SP<SHyprCtlCommand> g_queryCommand;

SDispatchResult callDispatcher(const std::string &name, const std::string &arg);
void refreshWorkspaceAlgorithms();

std::string fromFfiDirection(TilescriptDirection direction) {
  switch (direction) {
  case TILESCRIPT_DIRECTION_LEFT:
    return "left";
  case TILESCRIPT_DIRECTION_RIGHT:
    return "right";
  case TILESCRIPT_DIRECTION_UP:
    return "up";
  case TILESCRIPT_DIRECTION_DOWN:
    return "down";
  }

  return "left";
}

void registerTilescriptDispatchers() {
  tilescript_plugin::registerTilescriptDispatchers(
      PHANDLE, tilescript_plugin::DispatcherCallbacks{
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
  tilescript_plugin::refreshWorkspaceAlgorithms();
}

bool drainLayoutRuntimeSourceChangesCallback() {
  return tilescript_plugin::drainLayoutRuntimeSourceChanges();
}

void registerTilescriptAlgorithm() {
  tilescript_plugin::registerTilescriptAlgorithm(
      PHANDLE, tilescript_plugin::AlgorithmCallbacks{
                   .makeWindowId = makeWindowId,
                   .workspaceName = workspaceName,
               });
}

void unregisterTilescriptAlgorithm() {
  tilescript_plugin::unregisterTilescriptAlgorithm(PHANDLE);
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

SDispatchResult applyActions(const TilescriptActionResult &response) {
  if (response.error != nullptr) {
    return {.passEvent = false,
            .success = false,
            .error = std::string(response.error)};
  }

  for (size_t i = 0; i < response.action_count; ++i) {
    const auto &action = response.actions[i];
    SDispatchResult result;

    switch (action.kind) {
    case TILESCRIPT_ACTION_SPAWN_COMMAND:
      result = callDispatcher("exec",
                              action.string_value ? action.string_value : "");
      break;
    case TILESCRIPT_ACTION_RELOAD_CONFIG:
      HyprlandAPI::reloadConfig();
      break;
    case TILESCRIPT_ACTION_SET_LAYOUT:
      result = callDispatcher("layoutmsg",
                              "layout " + std::string(action.string_value
                                                          ? action.string_value
                                                          : ""));
      break;
    case TILESCRIPT_ACTION_CYCLE_LAYOUT:
      result =
          callDispatcher("layoutmsg", action.has_cycle_direction &&
                                              action.cycle_direction ==
                                                  TILESCRIPT_LAYOUT_CYCLE_PREVIOUS
                                          ? "cycleprev"
                                          : "cyclenext");
      break;
    case TILESCRIPT_ACTION_ACTIVATE_WORKSPACE:
      result = callDispatcher("workspace",
                              action.string_value ? action.string_value : "");
      break;
    case TILESCRIPT_ACTION_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE:
      result =
          callDispatcher("movetoworkspace", std::to_string(action.workspace));
      break;
    case TILESCRIPT_ACTION_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE:
      result = callDispatcher("movetoworkspacesilent",
                              std::to_string(action.workspace));
      break;
    case TILESCRIPT_ACTION_TOGGLE_FLOATING:
      result = callDispatcher("togglefloating", "");
      break;
    case TILESCRIPT_ACTION_TOGGLE_FULLSCREEN:
      result = callDispatcher("fullscreen", "1");
      break;
    case TILESCRIPT_ACTION_FOCUS_WINDOW:
      result = callDispatcher("focuswindow",
                              "address:" + std::string(action.string_value
                                                           ? action.string_value
                                                           : ""));
      break;
    case TILESCRIPT_ACTION_FOCUS_DIRECTION:
      result = callDispatcher("movefocus", fromFfiDirection(action.direction));
      break;
    case TILESCRIPT_ACTION_FOCUS_NEXT_WINDOW:
      result = callDispatcher("cyclenext", "");
      break;
    case TILESCRIPT_ACTION_FOCUS_PREVIOUS_WINDOW:
      result = callDispatcher("cyclenext", "prev");
      break;
    case TILESCRIPT_ACTION_SWAP_DIRECTION:
      result = callDispatcher("swapwindow", fromFfiDirection(action.direction));
      break;
    case TILESCRIPT_ACTION_MOVE_DIRECTION:
      result = callDispatcher("moveactive", fromFfiDirection(action.direction));
      break;
    case TILESCRIPT_ACTION_RESIZE_DIRECTION:
      result =
          callDispatcher("resizeactive", fromFfiDirection(action.direction));
      break;
    case TILESCRIPT_ACTION_CLOSE_FOCUSED_WINDOW:
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
  return tilescript_plugin::queryRuntime(format, std::move(arg), resyncAll);
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

  const auto hyprlandVersion = HyprlandAPI::getHyprlandVersion(PHANDLE);
  if (!hyprlandVersion.hash.empty() && hyprlandVersion.hash != GIT_COMMIT_HASH) {
    std::cerr << "[tilescript-hypr] Hyprland hash mismatch: plugin built for "
              << GIT_COMMIT_HASH << " but compositor is running " << hyprlandVersion.hash
              << std::endl;
    throw std::runtime_error("tilescript-hypr built against a different Hyprland revision");
  }

  HyprlandAPI::addConfigValue(PHANDLE, "plugin:tilescript-hypr:config_path",
                              Hyprlang::CConfigValue(""));
  setConfigPathValue(
      HyprlandAPI::getConfigValue(PHANDLE, "plugin:tilescript-hypr:config_path"));

  createRuntime();
  resyncAll();
  loadLayoutRuntimeConfig();
  if (layoutRuntimeLoaded()) {
    registerTilescriptAlgorithm();
    refreshWorkspaceAlgorithms();
  }
  registerTilescriptDispatchers();
  tilescript_plugin::registerHooks({
      .drainLayoutRuntimeSourceChanges = drainLayoutRuntimeSourceChangesCallback,
      .loadLayoutRuntimeConfig = loadLayoutRuntimeConfig,
      .layoutRuntimeLoaded = layoutRuntimeLoaded,
      .resyncAll = resyncAll,
      .registerTilescriptAlgorithm = registerTilescriptAlgorithm,
      .refreshWorkspaceAlgorithms = refreshWorkspaceAlgorithms,
  });

  g_queryCommand =
      HyprlandAPI::registerHyprCtlCommand(PHANDLE, SHyprCtlCommand{
                                                       .name = "tilescript-hypr",
                                                       .exact = false,
                                                       .fn = queryRuntime,
                                                   });

  if (!g_queryCommand) {
    std::cerr << "[tilescript-hypr] failed to register hyprctl command: tilescript-hypr"
              << std::endl;
  } else {
    std::cout << "[tilescript-hypr] registered hyprctl command: tilescript-hypr" << std::endl;
  }

  HyprlandAPI::addNotificationV2(PHANDLE,
                                 {
                                      {"text", std::string{"tilescript-hypr loaded"}},
                                     {"time", static_cast<uint64_t>(3000)},
                                     {"icon", ICON_INFO},
                                 });

  return {
      .name = "tilescript-hypr",
      .description = "Hyprland plugin bridge for tilescript",
      .author = "AkisArou",
      .version = "0.1.0",
  };
}

extern "C" EXPORT void pluginExit() {
  if (PHANDLE != nullptr) {
    if (g_queryCommand) {
      if (!HyprlandAPI::unregisterHyprCtlCommand(PHANDLE, g_queryCommand)) {
        std::cerr << "[tilescript-hypr] failed to unregister hyprctl command: tilescript-hypr"
                  << std::endl;
      } else {
        std::cout << "[tilescript-hypr] unregistered hyprctl command: tilescript-hypr"
                  << std::endl;
      }
      g_queryCommand.reset();
    }
  }

  clearHooks();
  clearSyncState();
  clearConfigPathValue();
  unregisterTilescriptAlgorithm();
  destroyRuntime();
  clearPluginHandle();
  PHANDLE = nullptr;
}

#ifdef __clang__
#pragma clang diagnostic pop
#endif
