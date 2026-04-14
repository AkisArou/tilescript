#include "hypreact_plugin_query.hpp"

#include <json/json.h>

#include "hypreact_hypr_ffi.h"
#include "hypreact_plugin_runtime.hpp"

namespace hypreact_plugin {
namespace {

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

void appendDiagnostics(Json::Value &target,
                       const HypreactDiagnostic *diagnostics,
                       size_t diagnosticCount) {
  for (size_t i = 0; i < diagnosticCount; ++i) {
    const auto &diagnostic = diagnostics[i];
    Json::Value item;
    if (diagnostic.source != nullptr) {
      item["source"] = diagnostic.source;
    }
    if (diagnostic.severity != nullptr) {
      item["severity"] = diagnostic.severity;
    }
    if (diagnostic.code != nullptr) {
      item["code"] = diagnostic.code;
    }
    if (diagnostic.message != nullptr) {
      item["message"] = diagnostic.message;
    }
    if (diagnostic.path != nullptr) {
      item["path"] = diagnostic.path;
    }

    Json::Value range;
    range["startLine"] = diagnostic.range.start_line;
    range["startColumn"] = diagnostic.range.start_column;
    range["endLine"] = diagnostic.range.end_line;
    range["endColumn"] = diagnostic.range.end_column;
    item["range"] = std::move(range);

    target.append(std::move(item));
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
  if (layout.diagnostics != nullptr && layout.diagnostic_count > 0) {
    appendDiagnostics(target["diagnostics"], layout.diagnostics,
                      layout.diagnostic_count);
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

} // namespace

std::string queryRuntime(eHyprCtlOutputFormat, std::string arg,
                         void (*resyncAll)()) {
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

} // namespace hypreact_plugin
