#include "hypreact_plugin_runtime.hpp"

#include <array>
#include <cctype>
#include <filesystem>
#include <iostream>
#include <memory>
#include <optional>
#include <sstream>
#include <string>

#include "src/plugins/PluginAPI.hpp"

namespace hypreact_plugin {

namespace {

std::unique_ptr<Runtime> g_runtime;
HANDLE g_pluginHandle = nullptr;
Hyprlang::CConfigValue* g_configPathConfig = nullptr;
std::optional<std::filesystem::path> g_resolvedConfigRoot;
std::string g_lastDiagnosticNotificationKey;

void notifyHypreact(const std::string& text, uint64_t icon = ICON_WARNING, uint64_t time = 5000) {
    if (g_pluginHandle == nullptr) {
        return;
    }

    HyprlandAPI::addNotificationV2(g_pluginHandle, {
        {"text", text},
        {"time", time},
        {"icon", icon},
    });
}

void notifyDiagnostics(const HypreactLayoutStatusResult& layout) {
    std::string key;
    if (layout.error != nullptr) {
        key.append("error:");
        key.append(layout.error);
    }
    if (layout.diagnostics_json != nullptr) {
        key.append("|diagnostics:");
        key.append(layout.diagnostics_json);
    }

    if (key.empty() || key == g_lastDiagnosticNotificationKey) {
        return;
    }

    g_lastDiagnosticNotificationKey = key;

    if (layout.error != nullptr) {
        notifyHypreact(std::string{"hypreact: "} + layout.error, ICON_ERROR, 8000);
    }

    if (layout.diagnostics_json == nullptr) {
        return;
    }

    const auto diagnostics = parseJson(layout.diagnostics_json);
    if (!diagnostics || !diagnostics->isArray()) {
        return;
    }

    for (const auto& diagnostic : *diagnostics) {
        const auto message = diagnostic["message"].asString();
        const auto path = diagnostic["path"].asString();
        const auto range = diagnostic["range"];
        std::ostringstream text;
        text << "hypreact css: " << message;
        if (!path.empty()) {
            text << " (" << path;
            if (range.isObject() && range["startLine"].isUInt()) {
                text << ":" << range["startLine"].asUInt();
            }
            text << ")";
        }

        const auto severity = diagnostic["severity"].asString();
        const auto icon = severity == "error" ? ICON_ERROR : ICON_WARNING;
        notifyHypreact(text.str(), icon, 7000);
    }
}

std::string configuredConfigPath() {
    if (!g_configPathConfig) {
        return {};
    }

    return trim(std::string{std::any_cast<Hyprlang::STRING>(g_configPathConfig->getValue())});
}

std::optional<std::filesystem::path> defaultConfigRoot() {
    const char* home = std::getenv("HOME");
    if (home == nullptr || std::string(home).empty()) {
        return std::nullopt;
    }

    return std::filesystem::path(home) / ".config" / "hypreact";
}

std::optional<std::filesystem::path> discoverConfigEntryInDirectory(const std::filesystem::path& root) {
    static const std::array<const char*, 4> candidates = {
        "config.ts",
        "config.tsx",
        "config.js",
        "config.jsx",
    };

    for (const auto* candidate : candidates) {
        const auto path = root / candidate;
        if (std::filesystem::exists(path) && std::filesystem::is_regular_file(path)) {
            return std::filesystem::canonical(path);
        }
    }

    return std::nullopt;
}

bool looksLikeConfigEntryPath(const std::filesystem::path& path) {
    const auto name = path.filename().string();
    return name == "config.ts" || name == "config.tsx" || name == "config.js" || name == "config.jsx";
}

std::optional<std::filesystem::path> resolveConfiguredConfigRoot() {
    std::error_code error;
    const auto configured = configuredConfigPath();

    if (!configured.empty()) {
        auto path = std::filesystem::path(configured);
        if (looksLikeConfigEntryPath(path)) {
            return std::nullopt;
        }
        if (!std::filesystem::exists(path, error)) {
            return path;
        }

        if (std::filesystem::is_directory(path, error)) {
            return std::filesystem::canonical(path, error);
        }

        return std::nullopt;
    }

    return defaultConfigRoot();
}

void syncSdkSupport(const std::filesystem::path& configRoot) {
    const auto result = hypreact_runtime_sync_sdk_support_result(configRoot.c_str());
    logStatusResult("sdk-sync", result);
    hypreact_runtime_free_status_result(result);
}

void bootstrapConfigRoot(const std::filesystem::path& configRoot) {
    const auto result = hypreact_runtime_bootstrap_config_result(configRoot.c_str());
    logStatusResult("config-bootstrap", result);
    hypreact_runtime_free_status_result(result);
}

} // namespace

Runtime::Runtime() : handle_(hypreact_runtime_new()) {}

Runtime::~Runtime() {
    if (handle_ != nullptr) {
        hypreact_runtime_free(handle_);
    }
}

std::string Runtime::resetState() const {
    return take(hypreact_runtime_reset_state(handle_));
}

std::string Runtime::upsertOutput(const HypreactOutputSync& output) const {
    return take(hypreact_runtime_upsert_output(handle_, &output));
}

std::string Runtime::removeOutput(const std::string& outputId) const {
    return take(hypreact_runtime_remove_output(handle_, outputId.c_str()));
}

std::string Runtime::activateWorkspace(const std::string& workspaceId, const std::string& outputId) const {
    return take(hypreact_runtime_activate_workspace(handle_, workspaceId.c_str(), outputId.empty() ? nullptr : outputId.c_str()));
}

std::string Runtime::setWorkspaceLayoutSpace(const HypreactWorkspaceLayoutSpaceSync& layoutSpace) const {
    return take(hypreact_runtime_set_workspace_layout_space(handle_, &layoutSpace));
}

std::string Runtime::focusWindow(const std::optional<std::string>& windowId) const {
    return take(hypreact_runtime_focus_window(handle_, windowId ? windowId->c_str() : nullptr));
}

std::string Runtime::setWindowClosing(const std::string& windowId, bool closing) const {
    return take(hypreact_runtime_set_window_closing(handle_, windowId.c_str(), closing));
}

std::string Runtime::removeWindow(const std::string& windowId) const {
    return take(hypreact_runtime_remove_window(handle_, windowId.c_str()));
}

std::string Runtime::upsertWindow(const HypreactWindowSync& window) const {
    return take(hypreact_runtime_upsert_window(handle_, &window));
}

HypreactStatusResult Runtime::loadLayoutConfig(const std::string& configPath) const {
    return hypreact_runtime_load_layout_config_result(handle_, configPath.c_str());
}

HypreactStatusResult Runtime::reloadLayoutConfig() const {
    return hypreact_runtime_reload_layout_config_result(handle_);
}

HypreactLayoutStatusResult Runtime::layoutStatusResult() const {
    return hypreact_runtime_layout_status_result(handle_);
}

HypreactPlacementResult Runtime::layoutPlacement() const {
    return hypreact_runtime_layout_placement(handle_);
}

HypreactPlacementResult Runtime::layoutPlacementForWorkspace(const std::string& workspaceId) const {
    return hypreact_runtime_layout_placement_for_workspace(handle_, workspaceId.c_str());
}

std::optional<std::string> Runtime::layoutFocusCandidate(const std::string& direction) const {
    const auto result = hypreact_runtime_layout_focus_candidate(handle_, direction.c_str());
    if (result.value == nullptr) {
        return std::nullopt;
    }

    std::string value(result.value);
    hypreact_string_free(result.value);
    return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
}

std::optional<std::string> Runtime::layoutCloseFocusCandidate(const std::string& windowId) const {
    const auto result = hypreact_runtime_layout_close_focus_candidate(handle_, windowId.c_str());
    if (result.value == nullptr) {
        return std::nullopt;
    }

    std::string value(result.value);
    hypreact_string_free(result.value);
    return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
}

std::optional<std::string> Runtime::layoutSwapCandidate(const std::string& direction) const {
    const auto result = hypreact_runtime_layout_swap_candidate(handle_, direction.c_str());
    if (result.value == nullptr) {
        return std::nullopt;
    }

    std::string value(result.value);
    hypreact_string_free(result.value);
    return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
}

bool Runtime::moveTiledWindow(const std::string& firstWindowId, const std::string& secondWindowId) const {
    const auto result = hypreact_runtime_move_tiled_window(handle_, firstWindowId.c_str(), secondWindowId.c_str());
    const auto changed = result.changed;
    hypreact_runtime_free_status_result(result);
    return changed;
}

bool Runtime::resizeDirection(const std::string& direction) const {
    const auto result = hypreact_runtime_resize_direction(handle_, direction.c_str());
    const auto changed = result.changed;
    hypreact_runtime_free_status_result(result);
    return changed;
}

HypreactStateResult Runtime::stateResult() const {
    return hypreact_runtime_state_result(handle_);
}

std::string Runtime::take(char* raw) {
    if (raw == nullptr) {
        return R"({"ok":false,"error":"ffi returned null"})";
    }

    std::string value(raw);
    hypreact_string_free(raw);
    return value;
}

Runtime* runtime() {
    return g_runtime.get();
}

void createRuntime() {
    g_runtime = std::make_unique<Runtime>();
}

void destroyRuntime() {
    g_runtime.reset();
    g_resolvedConfigRoot.reset();
    g_lastDiagnosticNotificationKey.clear();
}

void setPluginHandle(void* handle) {
    g_pluginHandle = handle;
}

void clearPluginHandle() {
    g_pluginHandle = nullptr;
}

void setConfigPathValue(Hyprlang::CConfigValue* value) {
    g_configPathConfig = value;
}

void clearConfigPathValue() {
    g_configPathConfig = nullptr;
}

void logJson(const char* label, const std::string& json) {
    std::cout << "[hypreact] " << label << ": " << json << std::endl;
}

void logStatusResult(const char* label, const HypreactStatusResult& result) {
    if (result.error != nullptr) {
        std::cerr << "[hypreact] " << label << " failed: " << result.error << std::endl;
    }
}

std::optional<Json::Value> parseJson(const std::string& json) {
    Json::CharReaderBuilder builder;
    std::string errors;
    Json::Value root;
    std::unique_ptr<Json::CharReader> reader(builder.newCharReader());
    const bool ok = reader->parse(json.data(), json.data() + json.size(), &root, &errors);
    if (!ok) {
        std::cerr << "[hypreact] failed to parse json: " << errors << std::endl;
        return std::nullopt;
    }

    return root;
}

std::string trim(std::string value) {
    while (!value.empty() && std::isspace(static_cast<unsigned char>(value.front()))) {
        value.erase(value.begin());
    }

    while (!value.empty() && std::isspace(static_cast<unsigned char>(value.back()))) {
        value.pop_back();
    }

    return value;
}

std::string stringify(const Json::Value& value) {
    Json::StreamWriterBuilder builder;
    builder["indentation"] = "";
    return Json::writeString(builder, value);
}

void loadLayoutRuntimeConfig() {
    if (!g_runtime) {
        return;
    }

    const auto resolvedRoot = resolveConfiguredConfigRoot();
    if (!resolvedRoot.has_value()) {
        g_resolvedConfigRoot.reset();
        return;
    }

    g_resolvedConfigRoot = *resolvedRoot;
    bootstrapConfigRoot(*g_resolvedConfigRoot);
    syncSdkSupport(*g_resolvedConfigRoot);

    const auto configEntry = discoverConfigEntryInDirectory(*g_resolvedConfigRoot);
    if (!configEntry.has_value()) {
        return;
    }

    const auto result = g_runtime->loadLayoutConfig(configEntry->string());
    logStatusResult("layout-runtime", result);
    hypreact_runtime_free_status_result(result);

    const auto layout = g_runtime->layoutStatusResult();
    notifyDiagnostics(layout);
    hypreact_runtime_free_layout_status_result(layout);
}

bool layoutRuntimeLoaded() {
    if (!g_runtime) {
        return false;
    }

    const auto status = g_runtime->layoutStatusResult();
    const auto loaded = status.loaded;
    hypreact_runtime_free_layout_status_result(status);
    return loaded;
}

} // namespace hypreact_plugin
