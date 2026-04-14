#pragma once

#include <memory>
#include <optional>
#include <string>

#include <json/json.h>

#include "hypreact_hypr_ffi.h"

#include "src/SharedDefs.hpp"

namespace Hyprlang {
class CConfigValue;
}

namespace hypreact_plugin {

class Runtime {
  public:
    Runtime();
    ~Runtime();

    [[nodiscard]] std::string resetState() const;
    [[nodiscard]] std::string upsertOutput(const HypreactOutputSync& output) const;
    [[nodiscard]] std::string removeOutput(const std::string& outputId) const;
    [[nodiscard]] std::string activateWorkspace(const std::string& workspaceId, const std::string& outputId) const;
    [[nodiscard]] std::string setWorkspaceLayoutSpace(const HypreactWorkspaceLayoutSpaceSync& layoutSpace) const;
    [[nodiscard]] std::string focusWindow(const std::optional<std::string>& windowId) const;
    [[nodiscard]] std::string setWindowClosing(const std::string& windowId, bool closing) const;
    [[nodiscard]] std::string removeWindow(const std::string& windowId) const;
    [[nodiscard]] std::string upsertWindow(const HypreactWindowSync& window) const;
    [[nodiscard]] HypreactStatusResult loadLayoutConfig(const std::string& configPath) const;
    [[nodiscard]] HypreactStatusResult reloadLayoutConfig() const;
    [[nodiscard]] HypreactLayoutStatusResult layoutStatusResult() const;
    [[nodiscard]] HypreactPlacementResult layoutPlacement() const;
    [[nodiscard]] HypreactPlacementResult layoutPlacementForWorkspace(const std::string& workspaceId) const;
    [[nodiscard]] std::optional<std::string> layoutFocusCandidate(const std::string& direction) const;
    [[nodiscard]] std::optional<std::string> layoutCloseFocusCandidate(const std::string& windowId) const;
    [[nodiscard]] std::optional<std::string> layoutSwapCandidate(const std::string& direction) const;
    [[nodiscard]] bool moveTiledWindow(const std::string& firstWindowId, const std::string& secondWindowId) const;
    [[nodiscard]] bool resizeDirection(const std::string& direction) const;
    [[nodiscard]] HypreactStateResult stateResult() const;

  private:
    static std::string take(char* raw);

    HypreactRuntimeHandle* handle_ = nullptr;
};

Runtime* runtime();
void createRuntime();
void destroyRuntime();

void setPluginHandle(void* handle);
void clearPluginHandle();
void setConfigPathValue(Hyprlang::CConfigValue* value);
void clearConfigPathValue();

void logJson(const char* label, const std::string& json);
void logStatusResult(const char* label, const HypreactStatusResult& result);
std::optional<Json::Value> parseJson(const std::string& json);
std::string trim(std::string value);
std::string stringify(const Json::Value& value);

void loadLayoutRuntimeConfig();
bool layoutRuntimeLoaded();

} // namespace hypreact_plugin
