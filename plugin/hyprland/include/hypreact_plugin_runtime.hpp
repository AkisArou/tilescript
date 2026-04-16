#pragma once

#include <functional>
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

    [[nodiscard]] HypreactStatusResult resetState() const;
    [[nodiscard]] HypreactStatusResult upsertOutput(const HypreactOutputSync& output) const;
    [[nodiscard]] HypreactStatusResult removeOutput(const std::string& outputId) const;
    [[nodiscard]] HypreactStatusResult activateWorkspace(const std::string& workspaceId, const std::string& outputId) const;
    [[nodiscard]] HypreactStatusResult setWorkspaceLayoutSpace(const HypreactWorkspaceLayoutSpaceSync& layoutSpace) const;
    [[nodiscard]] HypreactStatusResult focusWindow(const std::optional<std::string>& windowId) const;
    [[nodiscard]] HypreactStatusResult setWindowClosing(const std::string& windowId, bool closing) const;
    [[nodiscard]] HypreactStatusResult removeWindow(const std::string& windowId) const;
    [[nodiscard]] HypreactStatusResult upsertWindow(const HypreactWindowSync& window) const;
    [[nodiscard]] HypreactStatusResult loadLayoutConfig(const std::string& configPath) const;
    [[nodiscard]] HypreactStatusResult reloadLayoutConfig() const;
    [[nodiscard]] HypreactStatusResult drainLayoutSourceChanges() const;
    [[nodiscard]] int layoutSourceChangeFd() const;
    [[nodiscard]] HypreactLayoutStatusResult layoutStatusResult() const;
    [[nodiscard]] HypreactPlacementResult layoutPlacement() const;
    [[nodiscard]] HypreactPlacementResult layoutPlacementForWorkspace(const std::string& workspaceId) const;
    [[nodiscard]] std::optional<std::string> layoutFocusCandidate(const std::string& direction) const;
    [[nodiscard]] std::optional<std::string> layoutCloseFocusCandidate(const std::string& windowId) const;
    [[nodiscard]] std::optional<std::string> layoutSwapCandidate(const std::string& direction) const;
    [[nodiscard]] bool moveTiledWindow(const std::string& firstWindowId, const std::string& secondWindowId) const;
    [[nodiscard]] bool resizeDirection(const std::string& direction) const;
    [[nodiscard]] HypreactStateResult stateResult() const;

    HypreactRuntimeHandle* handle_ = nullptr;
};

Runtime* runtime();
void createRuntime();
void destroyRuntime();

void setPluginHandle(void* handle);
void clearPluginHandle();
void setConfigPathValue(Hyprlang::CConfigValue* value);
void clearConfigPathValue();

std::string trim(std::string value);
std::string stringify(const Json::Value& value);

void loadLayoutRuntimeConfig();
bool layoutRuntimeLoaded();
bool drainLayoutRuntimeSourceChanges();
void watchLayoutRuntimeSources(const std::function<void()>& callback);

} // namespace hypreact_plugin
