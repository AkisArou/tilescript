#pragma once

#include <functional>
#include <optional>
#include <string>

#include <json/json.h>

#include "tilescript_hypr_ffi.h"

#include "src/SharedDefs.hpp"

namespace Hyprlang {
class CConfigValue;
}

namespace tilescript_plugin {

class Runtime {
  public:
    Runtime();
    ~Runtime();

    [[nodiscard]] TilescriptStatusResult resetState() const;
    [[nodiscard]] TilescriptStatusResult upsertOutput(const TilescriptOutputSync& output) const;
    [[nodiscard]] TilescriptStatusResult removeOutput(const std::string& outputId) const;
    [[nodiscard]] TilescriptStatusResult activateWorkspace(const std::string& workspaceId, const std::string& outputId) const;
    [[nodiscard]] TilescriptStatusResult setWorkspaceLayoutSpace(const TilescriptWorkspaceLayoutSpaceSync& layoutSpace) const;
    [[nodiscard]] TilescriptStatusResult focusWindow(const std::optional<std::string>& windowId) const;
    [[nodiscard]] TilescriptStatusResult setWindowClosing(const std::string& windowId, bool closing) const;
    [[nodiscard]] TilescriptStatusResult removeWindow(const std::string& windowId) const;
    [[nodiscard]] TilescriptStatusResult upsertWindow(const TilescriptWindowSync& window) const;
    [[nodiscard]] TilescriptStatusResult loadLayoutConfig(const std::string& configPath) const;
    [[nodiscard]] TilescriptStatusResult reloadLayoutConfig() const;
    [[nodiscard]] TilescriptStatusResult drainLayoutSourceChanges() const;
    [[nodiscard]] int layoutSourceChangeFd() const;
    [[nodiscard]] TilescriptLayoutStatusResult layoutStatusResult() const;
    [[nodiscard]] TilescriptPlacementResult layoutPlacement() const;
    [[nodiscard]] TilescriptPlacementResult layoutPlacementForWorkspace(const std::string& workspaceId) const;
    [[nodiscard]] std::optional<std::string> layoutFocusCandidate(const std::string& direction) const;
    [[nodiscard]] std::optional<std::string> layoutCloseFocusCandidate(const std::string& windowId) const;
    [[nodiscard]] std::optional<std::string> layoutSwapCandidate(const std::string& direction) const;
    [[nodiscard]] bool moveTiledWindow(const std::string& firstWindowId, const std::string& secondWindowId) const;
    [[nodiscard]] bool resizeDirection(const std::string& direction) const;
    [[nodiscard]] TilescriptStateResult stateResult() const;

    TilescriptRuntimeHandle* handle_ = nullptr;
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

} // namespace tilescript_plugin
