#include <iostream>
#include <memory>
#include <optional>
#include <cctype>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

#include <json/json.h>

#include "hypreact_hypr_ffi.h"

#include "src/Compositor.hpp"
#include "src/SharedDefs.hpp"
#include "src/desktop/Workspace.hpp"
#include "src/desktop/state/FocusState.hpp"
#include "src/desktop/view/Window.hpp"
#include "src/event/EventBus.hpp"
#include "src/helpers/Monitor.hpp"
#include "src/layout/algorithm/Algorithm.hpp"
#include "src/layout/algorithm/tiled/master/MasterAlgorithm.hpp"
#include "src/layout/space/Space.hpp"
#include "src/layout/supplementary/WorkspaceAlgoMatcher.hpp"
#include "src/layout/target/Target.hpp"
#include "src/managers/KeybindManager.hpp"
#include "src/plugins/PluginAPI.hpp"

inline HANDLE PHANDLE = nullptr;

namespace {

class Runtime {
  public:
    Runtime() : handle_(hypreact_runtime_new()) {}

    ~Runtime() {
        if (handle_ != nullptr) {
            hypreact_runtime_free(handle_);
        }
    }

    [[nodiscard]] HypreactActionResult dispatchCommand(const HypreactCommandInput& command) const {
        return hypreact_runtime_dispatch_command(handle_, &command);
    }

    [[nodiscard]] HypreactActionResult dispatchCommandText(const std::string& command) const {
        return hypreact_runtime_dispatch_command_text(handle_, command.c_str());
    }

    [[nodiscard]] std::string resetState() const {
        return take(hypreact_runtime_reset_state(handle_));
    }

    [[nodiscard]] std::string upsertOutput(const HypreactOutputSync& output) const {
        return take(hypreact_runtime_upsert_output(handle_, &output));
    }

    [[nodiscard]] std::string removeOutput(const std::string& outputId) const {
        return take(hypreact_runtime_remove_output(handle_, outputId.c_str()));
    }

    [[nodiscard]] std::string activateWorkspace(const std::string& workspaceId, const std::string& outputId) const {
        return take(hypreact_runtime_activate_workspace(handle_, workspaceId.c_str(), outputId.empty() ? nullptr : outputId.c_str()));
    }

    [[nodiscard]] std::string focusWindow(const std::optional<std::string>& windowId) const {
        return take(hypreact_runtime_focus_window(handle_, windowId ? windowId->c_str() : nullptr));
    }

    [[nodiscard]] std::string removeWindow(const std::string& windowId) const {
        return take(hypreact_runtime_remove_window(handle_, windowId.c_str()));
    }

    [[nodiscard]] std::string upsertWindow(const HypreactWindowSync& window) const {
        return take(hypreact_runtime_upsert_window(handle_, &window));
    }

    [[nodiscard]] HypreactStatusResult loadLayoutConfig(const std::string& configPath) const {
        return hypreact_runtime_load_layout_config_result(handle_, configPath.c_str());
    }

    [[nodiscard]] HypreactStatusResult reloadLayoutConfig() const {
        return hypreact_runtime_reload_layout_config_result(handle_);
    }

    [[nodiscard]] HypreactLayoutStatusResult layoutStatusResult() const {
        return hypreact_runtime_layout_status_result(handle_);
    }

    [[nodiscard]] HypreactPlacementResult layoutPlacement() const {
        return hypreact_runtime_layout_placement(handle_);
    }

    [[nodiscard]] std::optional<std::string> layoutFocusCandidate(const std::string& direction) const {
        const auto result = hypreact_runtime_layout_focus_candidate(handle_, direction.c_str());
        if (result.value == nullptr) {
            return std::nullopt;
        }

        std::string value(result.value);
        hypreact_string_free(result.value);
        return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
    }

    [[nodiscard]] std::optional<std::string> layoutSwapCandidate(const std::string& direction) const {
        const auto result = hypreact_runtime_layout_swap_candidate(handle_, direction.c_str());
        if (result.value == nullptr) {
            return std::nullopt;
        }

        std::string value(result.value);
        hypreact_string_free(result.value);
        return value.empty() ? std::nullopt : std::optional<std::string>(std::move(value));
    }

    [[nodiscard]] HypreactStateResult stateResult() const {
        return hypreact_runtime_state_result(handle_);
    }

  private:
    static std::string take(char* raw) {
        if (raw == nullptr) {
            return R"({"ok":false,"error":"ffi returned null"})";
        }

        std::string value(raw);
        hypreact_string_free(raw);
        return value;
    }

    HypreactRuntimeHandle* handle_ = nullptr;
};

std::unique_ptr<Runtime> g_runtime;
std::vector<CHyprSignalListener> g_listeners;
SP<SHyprCtlCommand> g_queryCommand;
std::unordered_map<WINDOWID, std::string> g_windowIds;
struct PendingWorkspaceRecalculation {
    PHLWORKSPACE workspace;
    int remainingTicks;
};

struct WindowSyncPayload {
    std::string windowId;
    std::string workspaceId;
    std::string outputId;
    HypreactWindowSync ffi;
};

struct OutputSyncPayload {
    std::string outputId;
    std::string name;
    HypreactOutputSync ffi;
};

std::vector<PendingWorkspaceRecalculation> g_pendingWorkspaceRecalculations;
int g_pendingWorkspaceLayoutRefreshTicks = 0;
Hyprlang::CConfigValue* g_configPathConfig = nullptr;
bool g_registeredHypreactAlgo = false;

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

std::string configuredConfigPath() {
    if (!g_configPathConfig) {
        return {};
    }

    return trim(std::string{std::any_cast<Hyprlang::STRING>(g_configPathConfig->getValue())});
}

std::string stringify(const Json::Value& value) {
    Json::StreamWriterBuilder builder;
    builder["indentation"] = "";
    return Json::writeString(builder, value);
}

SDispatchResult callDispatcher(const std::string& name, const std::string& arg);
std::string makeWindowId(const PHLWINDOW& window);
std::string workspaceName(const PHLWORKSPACE& workspace);
void removeWindow(const PHLWINDOW& window);
void queueWorkspaceRecalculate(const PHLWORKSPACE& workspace);
void applyPlacementForWorkspace(const PHLWORKSPACE& workspace);

HypreactDirection toFfiDirection(const std::string& direction) {
    if (direction == "left") {
        return HYPREACT_DIRECTION_LEFT;
    }
    if (direction == "right") {
        return HYPREACT_DIRECTION_RIGHT;
    }
    if (direction == "up") {
        return HYPREACT_DIRECTION_UP;
    }
    return HYPREACT_DIRECTION_DOWN;
}

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

HypreactCommandInput makeCommandInput(HypreactCommandKind kind) {
    return HypreactCommandInput {
        .kind = kind,
        .string_value = nullptr,
        .workspace = 0,
        .direction = HYPREACT_DIRECTION_LEFT,
        .cycle_direction = HYPREACT_LAYOUT_CYCLE_NEXT,
        .has_cycle_direction = false,
    };
}

std::unordered_map<std::string, CBox> geometryMapFromPlacement(const HypreactPlacementResult& placement) {
    std::unordered_map<std::string, CBox> byWindowId;
    byWindowId.reserve(placement.geometry_count);
    for (size_t i = 0; i < placement.geometry_count; ++i) {
        const auto& entry = placement.geometries[i];
        if (entry.window_id == nullptr) {
            continue;
        }

        byWindowId.emplace(
            entry.window_id,
            CBox {
                static_cast<double>(entry.x),
                static_cast<double>(entry.y),
                static_cast<double>(entry.width),
                static_cast<double>(entry.height),
            }
        );
    }
    return byWindowId;
}

class CHypreactAlgorithm final : public Layout::ITiledAlgorithm {
  public:
    void newTarget(SP<Layout::ITarget> target) override {
        recalculate();
    }

    void movedTarget(SP<Layout::ITarget> target, std::optional<Vector2D> focalPoint = std::nullopt) override {
        recalculate();
    }

    void removeTarget(SP<Layout::ITarget> target) override {
        recalculate();
    }

    void resizeTarget(const Vector2D&, SP<Layout::ITarget>, Layout::eRectCorner = Layout::CORNER_NONE) override {}

    void recalculate() override {
        const auto parent = m_parent.lock();
        if (!parent || !g_runtime) {
            return;
        }

        const auto placement = g_runtime->layoutPlacement();
        const auto byWindowId = geometryMapFromPlacement(placement);
        hypreact_runtime_free_placement_result(placement);

        const auto space = parent->space();
        if (!space) {
            return;
        }

        for (const auto& weakTarget : space->targets()) {
            const auto target = weakTarget.lock();
            if (!target || target->floating() || !target->window()) {
                continue;
            }

            const auto windowId = makeWindowId(target->window());
            const auto it = byWindowId.find(windowId);
            if (it == byWindowId.end()) {
                continue;
            }

            if (it->second.w <= 0 || it->second.h <= 0) {
                continue;
            }

            target->setPositionGlobal(it->second);
        }
    }

    void swapTargets(SP<Layout::ITarget> a, SP<Layout::ITarget> b) override {
        a->swap(b);
        recalculate();
    }

    void moveTargetInDirection(SP<Layout::ITarget> t, Math::eDirection dir, bool silent) override {
        if (!t || !t->window()) {
            return;
        }

        const auto target = g_runtime->layoutFocusCandidate(Math::toString(dir));
        if (target.has_value()) {
            callDispatcher("focuswindow", "address:" + *target);
        }
    }

    SP<Layout::ITarget> getNextCandidate(SP<Layout::ITarget> old) override {
        const auto parent = m_parent.lock();
        if (!parent || !old || !old->window() || !g_runtime) {
            return old;
        }

        const auto placement = g_runtime->layoutPlacement();
        if (placement.geometry_count == 0 || placement.geometries == nullptr) {
            return old;
        }

        const auto currentId = makeWindowId(old->window());
        size_t currentIndex = placement.geometry_count;
        for (size_t i = 0; i < placement.geometry_count; ++i) {
            const auto windowId = placement.geometries[i].window_id;
            if (windowId != nullptr && currentId == windowId) {
                currentIndex = i;
                break;
            }
        }

        const auto space = parent->space();
        if (!space) {
            return old;
        }

        for (size_t step = 1; step <= placement.geometry_count; ++step) {
            const auto candidateIndex = currentIndex < placement.geometry_count ? (currentIndex + step) % placement.geometry_count : step - 1;
            const auto candidateId = placement.geometries[candidateIndex].window_id;
            if (candidateId == nullptr) {
                continue;
            }
            for (const auto& weakTarget : space->targets()) {
                const auto target = weakTarget.lock();
                if (!target || target->floating() || !target->window()) {
                    continue;
                }
                if (makeWindowId(target->window()) == candidateId) {
                    hypreact_runtime_free_placement_result(placement);
                    return target;
                }
            }
        }

        hypreact_runtime_free_placement_result(placement);
        return old;
    }
};

std::string makeWindowId(const PHLWINDOW& window) {
    const auto rawId = static_cast<WINDOWID>(reinterpret_cast<uintptr_t>(window.get()));
    const auto it = g_windowIds.find(rawId);
    if (it != g_windowIds.end()) {
        return it->second;
    }

    std::ostringstream stream;
    stream << "hypr-window-" << rawId;
    auto id = stream.str();
    g_windowIds.emplace(rawId, id);
    return id;
}

void forgetWindowId(const PHLWINDOW& window) {
    if (!window) {
        return;
    }

    const auto rawId = static_cast<WINDOWID>(reinterpret_cast<uintptr_t>(window.get()));
    g_windowIds.erase(rawId);
}

std::string workspaceName(const PHLWORKSPACE& workspace) {
    if (!workspace) {
        return "1";
    }

    return workspace->getConfigName();
}

std::string monitorId(const PHLMONITOR& monitor) {
    if (!monitor) {
        return "hyprland";
    }

    return monitor->m_name.empty() ? std::to_string(monitor->m_id) : monitor->m_name;
}

WindowSyncPayload makeUpsertWindowRequest(const PHLWINDOW& window) {
    const auto windowId = makeWindowId(window);
    const auto workspaceId = workspaceName(window->m_workspace);
    const auto outputId = monitorId(window->m_monitor.lock());

    auto payload = WindowSyncPayload {
        .windowId = windowId,
        .workspaceId = workspaceId,
        .outputId = outputId,
        .ffi = {
            .window_id = nullptr,
            .workspace_id = nullptr,
            .output_id = nullptr,
            .is_xwayland = window->m_isX11,
            .mapped = window->m_isMapped,
            .title = nullptr,
            .app_id = nullptr,
            .class_name = nullptr,
            .instance = nullptr,
            .role = nullptr,
            .window_type = nullptr,
            .urgent = window->m_isUrgent,
            .floating = window->m_isFloating,
            .fullscreen = window->isFullscreen(),
        },
    };

    payload.ffi.window_id = payload.windowId.c_str();
    payload.ffi.workspace_id = payload.workspaceId.empty() ? nullptr : payload.workspaceId.c_str();
    payload.ffi.output_id = payload.outputId.empty() ? nullptr : payload.outputId.c_str();
    payload.ffi.title = window->m_title.empty() ? nullptr : window->m_title.c_str();
    payload.ffi.app_id = window->m_class.empty() ? nullptr : window->m_class.c_str();
    payload.ffi.class_name = window->m_class.empty() ? nullptr : window->m_class.c_str();
    payload.ffi.instance = window->m_initialClass.empty() ? nullptr : window->m_initialClass.c_str();
    return payload;
}

void logSyncResponse(const std::string& response) {
    if (!g_runtime) {
        return;
    }

    logJson("sync", response);
}

void syncMonitor(const PHLMONITOR& monitor) {
    if (!monitor) {
        return;
    }

    auto payload = OutputSyncPayload {
        .outputId = monitorId(monitor),
        .name = monitor->m_name.empty() ? monitorId(monitor) : monitor->m_name,
        .ffi = {
            .output_id = nullptr,
            .name = nullptr,
            .logical_width = static_cast<int>(monitor->m_size.x) > 0 ? static_cast<unsigned int>(monitor->m_size.x) : 1920U,
            .logical_height = static_cast<int>(monitor->m_size.y) > 0 ? static_cast<unsigned int>(monitor->m_size.y) : 1080U,
        },
    };

    payload.ffi.output_id = payload.outputId.c_str();
    payload.ffi.name = payload.name.c_str();
    logSyncResponse(g_runtime->upsertOutput(payload.ffi));
}

void syncWorkspace(const PHLWORKSPACE& workspace, const PHLMONITOR& monitor) {
    if (!workspace || !g_runtime) {
        return;
    }

    logSyncResponse(g_runtime->activateWorkspace(workspaceName(workspace), monitorId(monitor)));
}

void syncWindow(const PHLWINDOW& window) {
    if (!window || !g_runtime) {
        return;
    }

    // Hyprland may emit open/update events for provisional window objects before they are
    // fully mapped. Keeping those placeholders in the runtime pollutes Spider's window set
    // and causes inconsistent placement while a new tiled target is opening.
    if (!window->m_isMapped) {
        removeWindow(window);
        return;
    }

    const auto payload = makeUpsertWindowRequest(window);
    logSyncResponse(g_runtime->upsertWindow(payload.ffi));
}

void recalculateWorkspace(const PHLWORKSPACE& workspace) {
    if (!workspace || !workspace->m_space) {
        return;
    }

    workspace->m_space->recheckWorkArea();
    workspace->m_space->recalculate();
    workspace->updateWindows();
    workspace->forceReportSizesToWindows();

    const auto monitor = workspace->m_monitor.lock();
    if (monitor && g_layoutManager) {
        g_layoutManager->recalculateMonitor(monitor);
    }

    applyPlacementForWorkspace(workspace);
}

void applyPlacementForWorkspace(const PHLWORKSPACE& workspace) {
    if (!workspace || !workspace->m_space || !g_runtime) {
        return;
    }

    const auto monitor = workspace->m_monitor.lock();
    if (!monitor || monitor->m_activeWorkspace != workspace) {
        return;
    }

    const auto placement = g_runtime->layoutPlacement();
    const auto byWindowId = geometryMapFromPlacement(placement);
    hypreact_runtime_free_placement_result(placement);

    for (const auto& window : g_pCompositor->m_windows) {
        if (!window || !window->m_isMapped || window->m_isFloating || !window->m_target) {
            continue;
        }

        if (window->m_workspace != workspace) {
            continue;
        }

        const auto it = byWindowId.find(makeWindowId(window));
        if (it == byWindowId.end() || it->second.w <= 0 || it->second.h <= 0) {
            continue;
        }

        window->m_target->setPositionGlobal(it->second);
    }

    workspace->updateWindows();
    workspace->forceReportSizesToWindows();
}

void queueWorkspaceRecalculate(const PHLWORKSPACE& workspace) {
    if (!workspace) {
        return;
    }

    for (auto& pending : g_pendingWorkspaceRecalculations) {
        if (pending.workspace.get() == workspace.get()) {
            pending.remainingTicks = std::max(pending.remainingTicks, 4);
            return;
        }
    }

    g_pendingWorkspaceRecalculations.push_back(PendingWorkspaceRecalculation {
        .workspace = workspace,
        .remainingTicks = 4,
    });

    g_pendingWorkspaceLayoutRefreshTicks = std::max(g_pendingWorkspaceLayoutRefreshTicks, 4);
}

void flushPendingWorkspaceRecalculations() {
    if (g_pendingWorkspaceLayoutRefreshTicks > 0) {
        Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
        --g_pendingWorkspaceLayoutRefreshTicks;
    }

    std::vector<PendingWorkspaceRecalculation> stillPending;
    stillPending.reserve(g_pendingWorkspaceRecalculations.size());

    for (auto pending : g_pendingWorkspaceRecalculations) {
        if (pending.workspace && !pending.workspace->inert()) {
            recalculateWorkspace(pending.workspace);

            const auto monitor = pending.workspace->m_monitor.lock();
            if (monitor && monitor->m_activeWorkspace == pending.workspace) {
                callDispatcher("workspace", workspaceName(pending.workspace));
            }

            if (--pending.remainingTicks > 0) {
                stillPending.push_back(std::move(pending));
            }
        }
    }

    g_pendingWorkspaceRecalculations = std::move(stillPending);
}

void recalculateWindowWorkspace(const PHLWINDOW& window) {
    if (!window) {
        return;
    }

    queueWorkspaceRecalculate(window->m_workspace);
}

void syncFocusedWindow(const PHLWINDOW& window) {
    if (!g_runtime) {
        return;
    }

    logSyncResponse(g_runtime->focusWindow(window ? std::optional<std::string>(makeWindowId(window)) : std::nullopt));
}

void removeWindow(const PHLWINDOW& window) {
    if (!window || !g_runtime) {
        return;
    }

    logSyncResponse(g_runtime->removeWindow(makeWindowId(window)));
    forgetWindowId(window);
}

void resyncAll() {
    if (!g_runtime) {
        return;
    }

    g_windowIds.clear();
    logSyncResponse(g_runtime->resetState());

    for (const auto& monitor : g_pCompositor->m_monitors) {
        syncMonitor(monitor);
        if (monitor && monitor->m_activeWorkspace) {
            syncWorkspace(monitor->m_activeWorkspace, monitor);
        }
    }

    for (const auto& window : g_pCompositor->m_windows) {
        if (window && window->m_isMapped) {
            syncWindow(window);
        }
    }

    if (const auto focus = Desktop::focusState()) {
        syncFocusedWindow(focus->window());
    }

    for (const auto& monitor : g_pCompositor->m_monitors) {
        if (monitor && monitor->m_activeWorkspace) {
            queueWorkspaceRecalculate(monitor->m_activeWorkspace);
        }
    }
}

void loadLayoutRuntimeConfig() {
    if (!g_runtime) {
        return;
    }

    const auto configPath = configuredConfigPath();
    if (configPath.empty()) {
        return;
    }

    const auto result = g_runtime->loadLayoutConfig(configPath);
    logStatusResult("layout-runtime", result);
    hypreact_runtime_free_status_result(result);
}

void registerHypreactAlgorithm() {
    if (g_registeredHypreactAlgo) {
        return;
    }

    g_registeredHypreactAlgo = HyprlandAPI::addTiledAlgo(
        PHANDLE,
        "hypreact",
        &typeid(CHypreactAlgorithm),
        [] { return makeUnique<CHypreactAlgorithm>(); }
    );

    if (g_registeredHypreactAlgo) {
        std::cout << "[hypreact] registered tiled algorithm: hypreact" << std::endl;
        Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
    } else {
        std::cerr << "[hypreact] failed to register tiled algorithm: hypreact" << std::endl;
    }
}

void unregisterHypreactAlgorithm() {
    if (!g_registeredHypreactAlgo) {
        return;
    }

    if (!HyprlandAPI::removeAlgo(PHANDLE, "hypreact")) {
        std::cerr << "[hypreact] failed to unregister tiled algorithm: hypreact" << std::endl;
        return;
    }

    std::cout << "[hypreact] unregistered tiled algorithm: hypreact" << std::endl;
    g_registeredHypreactAlgo = false;
}

SDispatchResult callDispatcher(const std::string& name, const std::string& arg) {
    const auto it = g_pKeybindManager->m_dispatchers.find(name);
    if (it == g_pKeybindManager->m_dispatchers.end()) {
        return {.passEvent = false, .success = false, .error = "unknown dispatcher: " + name};
    }

    return it->second(arg);
}

std::optional<std::string> normalizeDirection(const std::string& arg) {
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

std::optional<unsigned> parseWorkspaceNumber(const std::string& arg) {
    const auto value = trim(arg);
    if (value.empty()) {
        return std::nullopt;
    }

    for (const auto ch : value) {
        if (!std::isdigit(static_cast<unsigned char>(ch))) {
            return std::nullopt;
        }
    }

    try {
        return static_cast<unsigned>(std::stoul(value));
    } catch (...) {
        return std::nullopt;
    }
}

SDispatchResult applyActions(const HypreactActionResult& response) {
    if (response.error != nullptr) {
        return {.passEvent = false, .success = false, .error = std::string(response.error)};
    }

    for (size_t i = 0; i < response.action_count; ++i) {
        const auto& action = response.actions[i];
        SDispatchResult result;

        switch (action.kind) {
            case HYPREACT_ACTION_SPAWN_COMMAND:
                result = callDispatcher("exec", action.string_value ? action.string_value : "");
                break;
            case HYPREACT_ACTION_RELOAD_CONFIG:
                HyprlandAPI::reloadConfig();
                break;
            case HYPREACT_ACTION_SET_LAYOUT:
                result = callDispatcher("layoutmsg", "layout " + std::string(action.string_value ? action.string_value : ""));
                break;
            case HYPREACT_ACTION_CYCLE_LAYOUT:
                result = callDispatcher(
                    "layoutmsg",
                    action.has_cycle_direction && action.cycle_direction == HYPREACT_LAYOUT_CYCLE_PREVIOUS
                        ? "cycleprev"
                        : "cyclenext"
                );
                break;
            case HYPREACT_ACTION_ACTIVATE_WORKSPACE:
                result = callDispatcher("workspace", action.string_value ? action.string_value : "");
                break;
            case HYPREACT_ACTION_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE:
                result = callDispatcher("movetoworkspace", std::to_string(action.workspace));
                break;
            case HYPREACT_ACTION_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE:
                result = callDispatcher("movetoworkspacesilent", std::to_string(action.workspace));
                break;
            case HYPREACT_ACTION_TOGGLE_FLOATING:
                result = callDispatcher("togglefloating", "");
                break;
            case HYPREACT_ACTION_TOGGLE_FULLSCREEN:
                result = callDispatcher("fullscreen", "1");
                break;
            case HYPREACT_ACTION_FOCUS_WINDOW:
                result = callDispatcher("focuswindow", "address:" + std::string(action.string_value ? action.string_value : ""));
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
                result = callDispatcher("resizeactive", fromFfiDirection(action.direction));
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

void registerHooks() {
    auto& events = Event::bus()->m_events;

    g_listeners.push_back(events.tick.listen([] {
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
        if (g_runtime && monitor) {
            logSyncResponse(g_runtime->removeOutput(monitorId(monitor)));
        }
    }));

    g_listeners.push_back(events.monitor.focused.listen([](PHLMONITOR monitor) {
        if (monitor && monitor->m_activeWorkspace) {
            syncWorkspace(monitor->m_activeWorkspace, monitor);
            queueWorkspaceRecalculate(monitor->m_activeWorkspace);
        }
    }));

    g_listeners.push_back(events.window.open.listen([](PHLWINDOW window) {
        syncWindow(window);
    }));

    g_listeners.push_back(events.window.close.listen([](PHLWINDOW window) {
        removeWindow(window);
    }));

    g_listeners.push_back(events.window.destroy.listen([](PHLWINDOW window) {
        removeWindow(window);
    }));

    g_listeners.push_back(events.window.active.listen([](PHLWINDOW window, Desktop::eFocusReason) {
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

    g_listeners.push_back(events.window.fullscreen.listen([](PHLWINDOW window) {
        syncWindow(window);
    }));

    g_listeners.push_back(events.window.urgent.listen([](PHLWINDOW window) {
        syncWindow(window);
    }));

    g_listeners.push_back(events.window.pin.listen([](PHLWINDOW window) {
        syncWindow(window);
    }));

    g_listeners.push_back(events.window.moveToWorkspace.listen([](PHLWINDOW window, PHLWORKSPACE workspace) {
        syncWindow(window);
        syncWorkspace(workspace, window ? window->m_monitor.lock() : nullptr);
        queueWorkspaceRecalculate(workspace);
    }));

    g_listeners.push_back(events.workspace.active.listen([](PHLWORKSPACE workspace) {
        syncWorkspace(workspace, workspace ? workspace->m_monitor.lock() : nullptr);
        queueWorkspaceRecalculate(workspace);
    }));

    g_listeners.push_back(events.config.reloaded.listen([] {
        if (g_runtime) {
            const auto command = makeCommandInput(HYPREACT_COMMAND_RELOAD_CONFIG);
            const auto response = g_runtime->dispatchCommand(command);
            const auto result = applyActions(response);
            hypreact_runtime_free_action_result(response);
            if (!result.success) {
                std::cerr << "[hypreact] reload-config dispatch failed: " << result.error << std::endl;
            }
        }
        loadLayoutRuntimeConfig();
        Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
        resyncAll();
        flushPendingWorkspaceRecalculations();
    }));
}

std::string queryRuntime(eHyprCtlOutputFormat, std::string arg) {
    if (!g_runtime) {
        return R"({"ok":false,"error":"runtime not initialized"})";
    }

    const auto command = trim(arg);
    if (command == "resync") {
        resyncAll();
        return R"({"ok":true,"data":{"message":"resynced"}})";
    }

    if (command == "layouts") {
        loadLayoutRuntimeConfig();
        const auto layout = g_runtime->layoutStatusResult();
        Json::Value response;
        response["ok"] = true;
        response["data"]["loaded"] = layout.loaded;
        if (layout.config_path != nullptr) {
            response["data"]["configPath"] = layout.config_path;
        }
        if (layout.selected_layout_name != nullptr) {
            response["data"]["selectedLayoutName"] = layout.selected_layout_name;
        }
        if (layout.error != nullptr) {
            response["data"]["error"] = layout.error;
        }
        for (size_t i = 0; i < layout.workspace_name_count; ++i) {
            if (layout.workspace_names[i] != nullptr) {
                response["data"]["workspaceNames"].append(layout.workspace_names[i]);
            }
        }
        hypreact_runtime_free_layout_status_result(layout);
        return stringify(response);
    }

    if (command == "reload-layouts") {
        loadLayoutRuntimeConfig();
        const auto result = g_runtime->reloadLayoutConfig();
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

    const auto state = g_runtime->stateResult();
    if (state.current_workspace_id != nullptr) {
        response["data"]["runtime"]["currentWorkspaceId"] = state.current_workspace_id;
    }
    if (state.current_output_id != nullptr) {
        response["data"]["runtime"]["currentOutputId"] = state.current_output_id;
    }
    if (state.focused_window_id != nullptr) {
        response["data"]["runtime"]["focusedWindowId"] = state.focused_window_id;
    }
    for (size_t i = 0; i < state.workspace_name_count; ++i) {
        if (state.workspace_names[i] != nullptr) {
            response["data"]["runtime"]["workspaceNames"].append(state.workspace_names[i]);
        }
    }
    hypreact_runtime_free_state_result(state);

    loadLayoutRuntimeConfig();
    const auto layout = g_runtime->layoutStatusResult();
    response["data"]["layouts"]["loaded"] = layout.loaded;
    if (layout.config_path != nullptr) {
        response["data"]["layouts"]["configPath"] = layout.config_path;
    }
    if (layout.selected_layout_name != nullptr) {
        response["data"]["layouts"]["selectedLayoutName"] = layout.selected_layout_name;
    }
    if (layout.error != nullptr) {
        response["data"]["layouts"]["error"] = layout.error;
    }
    for (size_t i = 0; i < layout.workspace_name_count; ++i) {
        if (layout.workspace_names[i] != nullptr) {
            response["data"]["layouts"]["workspaceNames"].append(layout.workspace_names[i]);
        }
    }
    hypreact_runtime_free_layout_status_result(layout);

    return stringify(response);
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

    HyprlandAPI::addConfigValue(PHANDLE, "plugin:hypreact:config_path", Hyprlang::CConfigValue(""));
    g_configPathConfig = HyprlandAPI::getConfigValue(PHANDLE, "plugin:hypreact:config_path");

    g_runtime = std::make_unique<Runtime>();
    resyncAll();
    loadLayoutRuntimeConfig();
    registerHypreactAlgorithm();
    registerHooks();

    g_queryCommand = HyprlandAPI::registerHyprCtlCommand(PHANDLE, SHyprCtlCommand {
        .name = "hypreact",
        .exact = false,
        .fn = queryRuntime,
    });

    if (!g_queryCommand) {
        std::cerr << "[hypreact] failed to register hyprctl command: hypreact" << std::endl;
    } else {
        std::cout << "[hypreact] registered hyprctl command: hypreact" << std::endl;
    }

    HyprlandAPI::addNotificationV2(PHANDLE, {
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
                std::cerr << "[hypreact] failed to unregister hyprctl command: hypreact" << std::endl;
            } else {
                std::cout << "[hypreact] unregistered hyprctl command: hypreact" << std::endl;
            }
            g_queryCommand.reset();
        }
    }

    g_listeners.clear();
    g_pendingWorkspaceRecalculations.clear();
    g_pendingWorkspaceLayoutRefreshTicks = 0;
    g_windowIds.clear();
    g_configPathConfig = nullptr;
    unregisterHypreactAlgorithm();
    g_runtime.reset();
    PHANDLE = nullptr;
}

#ifdef __clang__
#pragma clang diagnostic pop
#endif
