#include "tilescript_hypr_plugin_runtime.hpp"

#include <array>
#include <cctype>
#include <cstring>
#include <filesystem>
#include <iostream>
#include <memory>
#include <optional>
#include <spawn.h>
#include <sstream>
#include <string>
#include <sys/wait.h>
#include <thread>
#include <vector>

#include <hyprutils/os/FileDescriptor.hpp>

#include "src/plugins/PluginAPI.hpp"
#include "src/managers/eventLoop/EventLoopManager.hpp"

extern char** environ;

namespace tilescript_plugin {

namespace {

std::unique_ptr<Runtime> g_runtime;
HANDLE g_pluginHandle = nullptr;
Hyprlang::CConfigValue* g_configPathConfig = nullptr;
std::optional<std::filesystem::path> g_resolvedConfigRoot;
std::string g_lastDiagnosticNotificationKey;
bool g_hasPersistentErrorBanner = false;
std::string g_lastPersistentErrorBannerText;
size_t g_layoutSourceWatchGeneration = 0;

void spawnHyprctlCommand(std::vector<std::string> args) {
    std::thread([args = std::move(args)]() mutable {
        std::vector<char*> argv;
        argv.reserve(args.size() + 2);
        argv.push_back(const_cast<char*>("hyprctl"));
        for (auto& arg : args) {
            argv.push_back(arg.data());
        }
        argv.push_back(nullptr);

        pid_t pid = 0;
        const auto spawn_error = posix_spawnp(&pid, "hyprctl", nullptr, nullptr, argv.data(), environ);
        if (spawn_error != 0) {
            return;
        }

        int status = 0;
        while (waitpid(pid, &status, 0) == -1 && errno == EINTR) {
        }
    }).detach();
}

void logStatusResult(const char* label, const TilescriptStatusResult& result) {
    if (result.error != nullptr) {
        std::cerr << "[tilescript] " << label << " failed: " << result.error << std::endl;
    }
}

void notifyTilescript(const std::string& text, uint64_t icon = ICON_WARNING, uint64_t time = 5000) {
    if (g_pluginHandle == nullptr) {
        return;
    }

    HyprlandAPI::addNotificationV2(g_pluginHandle, {
        {"text", text},
        {"time", time},
        {"icon", icon},
    });
}

void notifyDiagnostics(const TilescriptLayoutStatusResult& layout) {
    auto setPersistentError = [](const std::string& text) {
        if (!g_pluginHandle || g_lastPersistentErrorBannerText == text) {
            return;
        }

        spawnHyprctlCommand({"seterror", "rgba(ff5555ff)", text});
        g_hasPersistentErrorBanner = true;
        g_lastPersistentErrorBannerText = text;
    };

    auto clearPersistentError = []() {
        if (!g_pluginHandle || !g_hasPersistentErrorBanner) {
            return;
        }

        spawnHyprctlCommand({"seterror", "disable"});
        g_hasPersistentErrorBanner = false;
        g_lastPersistentErrorBannerText.clear();
    };

    std::optional<std::string> persistentErrorText;
    if (layout.error != nullptr) {
        persistentErrorText = std::string{"tilescript: "} + layout.error;
    } else {
        for (size_t i = 0; i < layout.diagnostic_count; ++i) {
            const auto& diagnostic = layout.diagnostics[i];
            if (diagnostic.severity == nullptr || std::string_view(diagnostic.severity) != "error") {
                continue;
            }

            const auto message = diagnostic.message == nullptr ? "" : diagnostic.message;
            const std::string path = diagnostic.path == nullptr ? "" : diagnostic.path;
            const auto source = diagnostic.source == nullptr ? "diagnostic" : diagnostic.source;
            std::ostringstream text;
            text << "tilescript " << source << ": " << message;
            if (!path.empty()) {
                text << " (" << path;
                if (diagnostic.range.start_line > 0) {
                    text << ":" << diagnostic.range.start_line;
                }
                text << ")";
            }
            persistentErrorText = text.str();
            break;
        }
    }

    if (persistentErrorText.has_value()) {
        setPersistentError(*persistentErrorText);
    } else {
        clearPersistentError();
    }

    std::string key;
    if (layout.error != nullptr) {
        key.append("error:");
        key.append(layout.error);
    }
    for (size_t i = 0; i < layout.diagnostic_count; ++i) {
        const auto& diagnostic = layout.diagnostics[i];
        key.append("|diagnostic:");
        if (diagnostic.severity != nullptr) {
            key.append(diagnostic.severity);
        }
        key.push_back(':');
        if (diagnostic.code != nullptr) {
            key.append(diagnostic.code);
        }
        key.push_back(':');
        if (diagnostic.message != nullptr) {
            key.append(diagnostic.message);
        }
        key.push_back(':');
        if (diagnostic.path != nullptr) {
            key.append(diagnostic.path);
        }
        key.push_back(':');
        key.append(std::to_string(diagnostic.range.start_line));
    }

    if (key.empty() || key == g_lastDiagnosticNotificationKey) {
        return;
    }

    g_lastDiagnosticNotificationKey = key;

    if (layout.error != nullptr) {
        notifyTilescript(std::string{"tilescript: "} + layout.error, ICON_ERROR, 8000);
    }

    if (layout.diagnostics == nullptr || layout.diagnostic_count == 0) {
        return;
    }

    for (size_t i = 0; i < layout.diagnostic_count; ++i) {
        const auto& diagnostic = layout.diagnostics[i];
        const auto message = diagnostic.message == nullptr ? "" : diagnostic.message;
        const std::string path = diagnostic.path == nullptr ? "" : diagnostic.path;
        const auto source = diagnostic.source == nullptr ? "diagnostic" : diagnostic.source;
        std::ostringstream text;
        text << "tilescript " << source << ": " << message;
        if (!path.empty()) {
            text << " (" << path;
            if (diagnostic.range.start_line > 0) {
                text << ":" << diagnostic.range.start_line;
            }
            text << ")";
        }

        const auto severity = diagnostic.severity == nullptr ? "" : diagnostic.severity;
        const auto icon = severity == "error" ? ICON_ERROR : ICON_WARNING;
        notifyTilescript(text.str(), icon, 7000);
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

    return std::filesystem::path(home) / ".config" / "tilescript";
}

std::optional<std::filesystem::path> discoverConfigEntryInDirectory(const std::filesystem::path& root) {
    static const std::array<const char*, 6> candidates = {
        "config.ts",
        "config.tsx",
        "config.js",
        "config.jsx",
        "config.lua",
        "config.fnl",
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
    return name == "config.ts" || name == "config.tsx" || name == "config.js" || name == "config.jsx"
        || name == "config.lua" || name == "config.fnl";
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
    const auto result = tilescript_runtime_sync_sdk_support_result(configRoot.c_str());
    logStatusResult("sdk-sync", result);
    tilescript_runtime_free_status_result(result);
}

void bootstrapConfigRoot(const std::filesystem::path& configRoot) {
    const auto result = tilescript_runtime_bootstrap_config_result(configRoot.c_str());
    logStatusResult("config-bootstrap", result);
    tilescript_runtime_free_status_result(result);
}

} // namespace

Runtime::Runtime() : handle_(tilescript_runtime_new()) {}

Runtime::~Runtime() {
    if (handle_ != nullptr) {
        tilescript_runtime_free(handle_);
    }
}

TilescriptStatusResult Runtime::resetState() const {
    return tilescript_runtime_reset_state_result(handle_);
}

TilescriptStatusResult Runtime::upsertOutput(const TilescriptOutputSync& output) const {
    return tilescript_runtime_upsert_output_result(handle_, &output);
}

TilescriptStatusResult Runtime::removeOutput(const std::string& outputId) const {
    return tilescript_runtime_remove_output_result(handle_, outputId.c_str());
}

TilescriptStatusResult Runtime::activateWorkspace(const std::string& workspaceId, const std::string& outputId) const {
    return tilescript_runtime_activate_workspace_result(handle_, workspaceId.c_str(), outputId.empty() ? nullptr : outputId.c_str());
}

TilescriptStatusResult Runtime::setWorkspaceLayoutSpace(const TilescriptWorkspaceLayoutSpaceSync& layoutSpace) const {
    return tilescript_runtime_set_workspace_layout_space_result(handle_, &layoutSpace);
}

TilescriptStatusResult Runtime::focusWindow(const std::optional<std::string>& windowId) const {
    return tilescript_runtime_focus_window_result(handle_, windowId ? windowId->c_str() : nullptr);
}

TilescriptStatusResult Runtime::setWindowClosing(const std::string& windowId, bool closing) const {
    return tilescript_runtime_set_window_closing_result(handle_, windowId.c_str(), closing);
}

TilescriptStatusResult Runtime::removeWindow(const std::string& windowId) const {
    return tilescript_runtime_remove_window_result(handle_, windowId.c_str());
}

TilescriptStatusResult Runtime::upsertWindow(const TilescriptWindowSync& window) const {
    return tilescript_runtime_upsert_window_result(handle_, &window);
}

TilescriptStatusResult Runtime::loadLayoutConfig(const std::string& configPath) const {
    return tilescript_runtime_load_layout_config_result(handle_, configPath.c_str());
}

TilescriptStatusResult Runtime::reloadLayoutConfig() const {
    return tilescript_runtime_reload_layout_config_result(handle_);
}

TilescriptStatusResult Runtime::drainLayoutSourceChanges() const {
    return tilescript_runtime_poll_layout_sources_result(handle_);
}

int Runtime::layoutSourceChangeFd() const {
    return tilescript_runtime_layout_source_change_fd(handle_);
}

TilescriptLayoutStatusResult Runtime::layoutStatusResult() const {
    return tilescript_runtime_layout_status_result(handle_);
}

TilescriptPlacementResult Runtime::layoutPlacement() const {
    const auto layout = tilescript_runtime_layout_status_result(handle_);
    notifyDiagnostics(layout);
    tilescript_runtime_free_layout_status_result(layout);
    return tilescript_runtime_layout_placement(handle_);
}

TilescriptPlacementResult Runtime::layoutPlacementForWorkspace(const std::string& workspaceId) const {
    const auto layout = tilescript_runtime_layout_status_result(handle_);
    notifyDiagnostics(layout);
    tilescript_runtime_free_layout_status_result(layout);
    return tilescript_runtime_layout_placement_for_workspace(handle_, workspaceId.c_str());
}

std::optional<std::string> Runtime::layoutFocusCandidate(const std::string& direction) const {
    const auto result = tilescript_runtime_layout_focus_candidate(handle_, direction.c_str());
    if (result.value == nullptr) {
        return std::nullopt;
    }

    std::string value(result.value);
    tilescript_string_free(result.value);
    return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
}

std::optional<std::string> Runtime::layoutCloseFocusCandidate(const std::string& windowId) const {
    const auto result = tilescript_runtime_layout_close_focus_candidate(handle_, windowId.c_str());
    if (result.value == nullptr) {
        return std::nullopt;
    }

    std::string value(result.value);
    tilescript_string_free(result.value);
    return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
}

std::optional<std::string> Runtime::layoutSwapCandidate(const std::string& direction) const {
    const auto result = tilescript_runtime_layout_swap_candidate(handle_, direction.c_str());
    if (result.value == nullptr) {
        return std::nullopt;
    }

    std::string value(result.value);
    tilescript_string_free(result.value);
    return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
}

bool Runtime::layoutMoveDirection(const std::string& direction) const {
    const auto result = tilescript_runtime_layout_move_direction(handle_, direction.c_str());
    const auto changed = result.changed;
    tilescript_runtime_free_status_result(result);
    return changed;
}

bool Runtime::moveTiledWindow(const std::string& firstWindowId, const std::string& secondWindowId) const {
    const auto result = tilescript_runtime_move_tiled_window(handle_, firstWindowId.c_str(), secondWindowId.c_str());
    const auto changed = result.changed;
    tilescript_runtime_free_status_result(result);
    return changed;
}

bool Runtime::resizeDirection(const std::string& direction) const {
    const auto result = tilescript_runtime_resize_direction(handle_, direction.c_str());
    const auto changed = result.changed;
    tilescript_runtime_free_status_result(result);
    return changed;
}

TilescriptStateResult Runtime::stateResult() const {
    return tilescript_runtime_state_result(handle_);
}

Runtime* runtime() {
    return g_runtime.get();
}

void createRuntime() {
    g_runtime = std::make_unique<Runtime>();
}

void destroyRuntime() {
    ++g_layoutSourceWatchGeneration;
    g_runtime.reset();
    g_resolvedConfigRoot.reset();
    g_lastDiagnosticNotificationKey.clear();
    g_hasPersistentErrorBanner = false;
    g_lastPersistentErrorBannerText.clear();
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

    ++g_layoutSourceWatchGeneration;

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
    tilescript_runtime_free_status_result(result);

    const auto layout = g_runtime->layoutStatusResult();
    notifyDiagnostics(layout);
    tilescript_runtime_free_layout_status_result(layout);
}

bool layoutRuntimeLoaded() {
    if (!g_runtime) {
        return false;
    }

    const auto status = g_runtime->layoutStatusResult();
    const auto loaded = status.loaded;
    tilescript_runtime_free_layout_status_result(status);
    return loaded;
}

bool drainLayoutRuntimeSourceChanges() {
    if (!g_runtime) {
        return false;
    }

    const auto result = g_runtime->drainLayoutSourceChanges();
    const bool changed = result.changed;
    logStatusResult("drain-layout-source-changes", result);
    tilescript_runtime_free_status_result(result);
    return changed;
}

void watchLayoutRuntimeSources(const std::function<void()>& callback) {
    if (!g_runtime || !g_pEventLoopManager) {
        return;
    }

    const auto generation = g_layoutSourceWatchGeneration;

    const auto fd = g_runtime->layoutSourceChangeFd();
    if (fd < 0) {
        return;
    }

    g_pEventLoopManager->doOnReadable(Hyprutils::OS::CFileDescriptor{fcntl(fd, F_DUPFD_CLOEXEC, 0)}, [callback, generation] {
        if (generation != g_layoutSourceWatchGeneration) {
            return;
        }
        callback();
        watchLayoutRuntimeSources(callback);
    });
}

} // namespace tilescript_plugin
