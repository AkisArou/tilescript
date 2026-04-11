#include <iostream>
#include <memory>
#include <optional>
#include <cctype>
#include <functional>
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

    [[nodiscard]] std::string sendCommand(const std::string& json) const {
        return take(hypreact_runtime_handle_command(handle_, json.c_str()));
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

    [[nodiscard]] std::string loadLayoutConfig(const std::string& configPath) const {
        return take(hypreact_runtime_load_layout_config(handle_, configPath.c_str()));
    }

    [[nodiscard]] std::string reloadLayoutConfig() const {
        return take(hypreact_runtime_reload_layout_config(handle_));
    }

    [[nodiscard]] std::string layoutStatus() const {
        return take(hypreact_runtime_layout_status(handle_));
    }

    [[nodiscard]] std::string layoutPlacement() const {
        return take(hypreact_runtime_layout_placement(handle_));
    }

    [[nodiscard]] std::string layoutFocusCandidate(const std::string& direction) const {
        return take(hypreact_runtime_layout_focus_candidate(handle_, direction.c_str()));
    }

    [[nodiscard]] std::string layoutSwapCandidate(const std::string& direction) const {
        return take(hypreact_runtime_layout_swap_candidate(handle_, direction.c_str()));
    }

    [[nodiscard]] std::string layoutResizeMaster(double delta) const {
        return take(hypreact_runtime_layout_resize_master(handle_, delta));
    }

    [[nodiscard]] std::string state() const {
        return take(hypreact_runtime_state(handle_));
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
std::unordered_map<std::string, std::function<SDispatchResult(std::string)>> g_originalDispatchers;
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
Hyprlang::CConfigValue* g_enabledDispatchersConfig = nullptr;
Hyprlang::CConfigValue* g_fallbackNativeConfig = nullptr;
Hyprlang::CConfigValue* g_configPathConfig = nullptr;
bool g_registeredSpidersAlgo = false;

void logJson(const char* label, const std::string& json) {
    std::cout << "[hypreact] " << label << ": " << json << std::endl;
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

std::vector<std::string> splitList(const std::string& value) {
    std::vector<std::string> items;
    std::stringstream stream(value);
    std::string item;
    while (std::getline(stream, item, ',')) {
        item = trim(item);
        if (!item.empty()) {
            items.push_back(item);
        }
    }
    return items;
}

bool nativeFallbackEnabled() {
    if (!g_fallbackNativeConfig) {
        return true;
    }

    return std::any_cast<Hyprlang::INT>(g_fallbackNativeConfig->getValue()) != 0;
}

std::vector<std::string> configuredDispatcherNames() {
    if (!g_enabledDispatchersConfig) {
        return {
            "exec",
            "movefocus",
            "swapwindow",
            "workspace",
            "movetoworkspace",
            "movetoworkspacesilent",
            "togglefloating",
            "fullscreen",
            "killactive",
            "cyclenext",
            "layoutmsg",
        };
    }

    const auto raw = std::string{std::any_cast<Hyprlang::STRING>(g_enabledDispatchersConfig->getValue())};
    const auto parsed = splitList(raw);
    if (!parsed.empty()) {
        return parsed;
    }

    return {};
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

    void resizeTarget(const Vector2D& Δ, SP<Layout::ITarget> target, Layout::eRectCorner corner = Layout::CORNER_NONE) override {
        const auto parent = m_parent.lock();
        if (!parent || !target || !target->window()) {
            return;
        }

        const auto workspace = parent->space() ? parent->space()->workspace() : nullptr;
        if (!workspace) {
            return;
        }

        const auto currentId = makeWindowId(target->window());
        const auto responseText = g_runtime->layoutPlacement();
        const auto response = parseJson(responseText);
        if (!response.has_value() || !(*response)["ok"].asBool()) {
            return;
        }

        const auto& ordered = (*response)["data"]["orderedWindowIds"];
        if (!ordered.isArray() || ordered.empty()) {
            return;
        }

        const bool isMaster = ordered[0].asString() == currentId;
        const auto workArea = parent->space()->workArea();
        if (workArea.w <= 0 || workArea.h <= 0) {
            return;
        }

        const bool verticalSplit = workArea.w >= workArea.h;
        const double delta = verticalSplit ? (Δ.x / workArea.w) : (Δ.y / workArea.h);
        const double signedDelta = isMaster ? delta : -delta;

        logJson("layout-resize", g_runtime->layoutResizeMaster(signedDelta));

        recalculate();
    }

    void recalculate() override {
        const auto parent = m_parent.lock();
        if (!parent || !g_runtime) {
            return;
        }

        const auto responseText = g_runtime->layoutPlacement();
        const auto response = parseJson(responseText);
        if (!response.has_value() || !(*response)["ok"].asBool()) {
            logJson("layout-recalculate", responseText);
            return;
        }

        const auto& geometries = (*response)["data"]["windowGeometries"];
        if (!geometries.isArray()) {
            return;
        }

        std::unordered_map<std::string, CBox> byWindowId;
        byWindowId.reserve(geometries.size());
        for (const auto& entry : geometries) {
            byWindowId.emplace(
                entry["windowId"].asString(),
                CBox {
                    static_cast<double>(entry["x"].asInt()),
                    static_cast<double>(entry["y"].asInt()),
                    static_cast<double>(entry["width"].asInt()),
                    static_cast<double>(entry["height"].asInt()),
                }
            );
        }

        const auto space = parent->space();
        if (!space) {
            return;
        }

        const auto workArea = space->workArea();
        const auto& ordered = (*response)["data"]["orderedWindowIds"];
        const auto ratioValue = (*response)["data"]["masterRatio"];

        if (ratioValue.isDouble() && ordered.isArray() && ordered.size() >= 2) {
            const auto masterId = ordered[0].asString();
            const bool verticalSplit = workArea.w >= workArea.h;
            const double ratio = std::clamp(ratioValue.asDouble(), 0.2, 0.8);

            for (Json::ArrayIndex i = 0; i < ordered.size(); ++i) {
                const auto id = ordered[i].asString();
                auto it = byWindowId.find(id);
                if (it == byWindowId.end()) {
                    continue;
                }

                if (id == masterId) {
                    if (verticalSplit) {
                        it->second = CBox {workArea.x, workArea.y, workArea.w * ratio, workArea.h};
                    } else {
                        it->second = CBox {workArea.x, workArea.y, workArea.w, workArea.h * ratio};
                    }
                    continue;
                }

                const auto stackCount = static_cast<double>(ordered.size() - 1);
                if (stackCount <= 0) {
                    continue;
                }

                if (verticalSplit) {
                    const double stackX = workArea.x + workArea.w * ratio;
                    const double stackW = workArea.w - workArea.w * ratio;
                    const double eachH = workArea.h / stackCount;
                    const double stackIndex = static_cast<double>(i - 1);
                    it->second = CBox {stackX, workArea.y + eachH * stackIndex, stackW, eachH};
                } else {
                    const double stackY = workArea.y + workArea.h * ratio;
                    const double stackH = workArea.h - workArea.h * ratio;
                    const double eachW = workArea.w / stackCount;
                    const double stackIndex = static_cast<double>(i - 1);
                    it->second = CBox {workArea.x + eachW * stackIndex, stackY, eachW, stackH};
                }
            }
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

        const auto responseText = g_runtime->layoutFocusCandidate(Math::toString(dir));
        const auto response = parseJson(responseText);
        if (!response.has_value() || !(*response)["ok"].asBool()) {
            return;
        }
        const auto& target = (*response)["data"];
        if (target.isString() && !target.asString().empty()) {
            callDispatcher("focuswindow", "address:" + target.asString());
        }
    }

    SP<Layout::ITarget> getNextCandidate(SP<Layout::ITarget> old) override {
        const auto parent = m_parent.lock();
        if (!parent || !old || !old->window() || !g_runtime) {
            return old;
        }

        const auto responseText = g_runtime->layoutPlacement();
        const auto response = parseJson(responseText);
        if (!response.has_value() || !(*response)["ok"].asBool()) {
            return old;
        }

        const auto& ordered = (*response)["data"]["orderedWindowIds"];
        if (!ordered.isArray() || ordered.empty()) {
            return old;
        }

        const auto currentId = makeWindowId(old->window());
        Json::ArrayIndex currentIndex = ordered.size();
        for (Json::ArrayIndex i = 0; i < ordered.size(); ++i) {
            if (ordered[i].asString() == currentId) {
                currentIndex = i;
                break;
            }
        }

        const auto space = parent->space();
        if (!space) {
            return old;
        }

        for (Json::ArrayIndex step = 1; step <= ordered.size(); ++step) {
            const auto candidateIndex = currentIndex < ordered.size() ? (currentIndex + step) % ordered.size() : step - 1;
            const auto candidateId = ordered[candidateIndex].asString();
            for (const auto& weakTarget : space->targets()) {
                const auto target = weakTarget.lock();
                if (!target || target->floating() || !target->window()) {
                    continue;
                }
                if (makeWindowId(target->window()) == candidateId) {
                    return target;
                }
            }
        }

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

    const auto responseText = g_runtime->layoutPlacement();
    const auto response = parseJson(responseText);
    if (!response.has_value() || !(*response)["ok"].asBool()) {
        return;
    }

    const auto& geometries = (*response)["data"]["windowGeometries"];
    if (!geometries.isArray()) {
        return;
    }

    std::unordered_map<std::string, CBox> byWindowId;
    byWindowId.reserve(geometries.size());
    for (const auto& entry : geometries) {
        byWindowId.emplace(
            entry["windowId"].asString(),
            CBox {
                static_cast<double>(entry["x"].asInt()),
                static_cast<double>(entry["y"].asInt()),
                static_cast<double>(entry["width"].asInt()),
                static_cast<double>(entry["height"].asInt()),
            }
        );
    }

    const auto workArea = workspace->m_space->workArea();
    const auto& ordered = (*response)["data"]["orderedWindowIds"];
    const auto ratioValue = (*response)["data"]["masterRatio"];

    if (ratioValue.isDouble() && ordered.isArray() && ordered.size() >= 2) {
        const auto masterId = ordered[0].asString();
        const bool verticalSplit = workArea.w >= workArea.h;
        const double ratio = std::clamp(ratioValue.asDouble(), 0.2, 0.8);

        for (Json::ArrayIndex i = 0; i < ordered.size(); ++i) {
            const auto id = ordered[i].asString();
            auto it = byWindowId.find(id);
            if (it == byWindowId.end()) {
                continue;
            }

            if (id == masterId) {
                if (verticalSplit) {
                    it->second = CBox {workArea.x, workArea.y, workArea.w * ratio, workArea.h};
                } else {
                    it->second = CBox {workArea.x, workArea.y, workArea.w, workArea.h * ratio};
                }
                continue;
            }

            const auto stackCount = static_cast<double>(ordered.size() - 1);
            if (stackCount <= 0) {
                continue;
            }

            if (verticalSplit) {
                const double stackX = workArea.x + workArea.w * ratio;
                const double stackW = workArea.w - workArea.w * ratio;
                const double eachH = workArea.h / stackCount;
                const double stackIndex = static_cast<double>(i - 1);
                it->second = CBox {stackX, workArea.y + eachH * stackIndex, stackW, eachH};
            } else {
                const double stackY = workArea.y + workArea.h * ratio;
                const double stackH = workArea.h - workArea.h * ratio;
                const double eachW = workArea.w / stackCount;
                const double stackIndex = static_cast<double>(i - 1);
                it->second = CBox {workArea.x + eachW * stackIndex, stackY, eachW, stackH};
            }
        }
    }

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

    logJson("layout-runtime", g_runtime->loadLayoutConfig(configPath));
}

void registerHypreactAlgorithm() {
    if (g_registeredSpidersAlgo) {
        return;
    }

    g_registeredSpidersAlgo = HyprlandAPI::addTiledAlgo(
        PHANDLE,
        "hypreact",
        &typeid(CHypreactAlgorithm),
        [] { return makeUnique<CHypreactAlgorithm>(); }
    );

    if (g_registeredSpidersAlgo) {
        std::cout << "[hypreact] registered tiled algorithm: hypreact" << std::endl;
        Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
    } else {
        std::cerr << "[hypreact] failed to register tiled algorithm: hypreact" << std::endl;
    }
}

void unregisterHypreactAlgorithm() {
    if (!g_registeredSpidersAlgo) {
        return;
    }

    if (!HyprlandAPI::removeAlgo(PHANDLE, "hypreact")) {
        std::cerr << "[hypreact] failed to unregister tiled algorithm: hypreact" << std::endl;
        return;
    }

    std::cout << "[hypreact] unregistered tiled algorithm: hypreact" << std::endl;
    g_registeredSpidersAlgo = false;
}

SDispatchResult callDispatcher(const std::string& name, const std::string& arg) {
    const auto it = g_originalDispatchers.find(name);
    if (it == g_originalDispatchers.end()) {
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

std::optional<Json::Value> commandFromDispatcher(const std::string& name, const std::string& arg) {
    Json::Value command;

    if (name == "exec") {
        command["type"] = "spawn";
        command["command"] = arg;
        return command;
    }

    if (name == "movefocus") {
        const auto direction = normalizeDirection(arg);
        if (!direction.has_value()) {
            return std::nullopt;
        }
        command["type"] = "focus-direction";
        command["direction"] = *direction;
        return command;
    }

    if (name == "swapwindow") {
        const auto direction = normalizeDirection(arg);
        if (!direction.has_value()) {
            return std::nullopt;
        }
        command["type"] = "swap-direction";
        command["direction"] = *direction;
        return command;
    }

    if (name == "workspace") {
        const auto value = trim(arg);
        if (value == "e+1" || value == "m+1" || value == "+1") {
            command["type"] = "select-next-workspace";
            return command;
        }
        if (value == "e-1" || value == "m-1" || value == "-1") {
            command["type"] = "select-previous-workspace";
            return command;
        }
        if (const auto workspace = parseWorkspaceNumber(value)) {
            command["type"] = "view-workspace";
            command["workspace"] = *workspace;
            return command;
        }
        if (!value.empty()) {
            command["type"] = "activate-workspace";
            command["workspace_id"] = value;
            return command;
        }
        return std::nullopt;
    }

    if (name == "movetoworkspace" || name == "movetoworkspacesilent") {
        if (const auto workspace = parseWorkspaceNumber(arg)) {
            command["type"] = name == "movetoworkspacesilent" ? "toggle-assign-focused-window-to-workspace" : "assign-focused-window-to-workspace";
            command["workspace"] = *workspace;
            return command;
        }
        return std::nullopt;
    }

    if (name == "togglefloating") {
        command["type"] = "toggle-floating";
        return command;
    }

    if (name == "fullscreen") {
        command["type"] = "toggle-fullscreen";
        return command;
    }

    if (name == "killactive") {
        command["type"] = "close-focused-window";
        return command;
    }

    if (name == "cyclenext") {
        command["type"] = trim(arg) == "prev" ? "focus-previous-window" : "focus-next-window";
        return command;
    }

    if (name == "layoutmsg") {
        const auto value = trim(arg);
        if (value == "cyclenext") {
            command["type"] = "cycle-layout";
            return command;
        }
        if (value == "cycleprev") {
            command["type"] = "cycle-layout";
            command["direction"] = "previous";
            return command;
        }
        constexpr auto prefix = "layout ";
        if (value.rfind(prefix, 0) == 0 && value.size() > std::char_traits<char>::length(prefix)) {
            command["type"] = "set-layout";
            command["name"] = value.substr(std::char_traits<char>::length(prefix));
            return command;
        }
        return std::nullopt;
    }

    return std::nullopt;
}

SDispatchResult applyActions(const Json::Value& response) {
    const auto& actions = response["data"]["actions"];
    if (!actions.isArray()) {
        return {};
    }

    for (const auto& action : actions) {
        const auto type = action["type"].asString();
        SDispatchResult result;

        if (type == "spawn-command") {
            result = callDispatcher("exec", action["command"].asString());
        } else if (type == "activate-workspace") {
            result = callDispatcher("workspace", action["workspaceId"].asString());
        } else if (type == "assign-focused-window-to-workspace") {
            result = callDispatcher("movetoworkspace", std::to_string(action["workspace"].asInt()));
        } else if (type == "toggle-assign-focused-window-to-workspace") {
            result = callDispatcher("movetoworkspacesilent", std::to_string(action["workspace"].asInt()));
        } else if (type == "focus-window") {
            result = callDispatcher("focuswindow", "address:" + action["windowId"].asString());
        } else if (type == "focus-direction") {
            result = callDispatcher("movefocus", action["direction"].asString());
        } else if (type == "focus-next-window") {
            result = callDispatcher("cyclenext", "");
        } else if (type == "focus-previous-window") {
            result = callDispatcher("cyclenext", "prev");
        } else if (type == "close-focused-window") {
            result = callDispatcher("killactive", "");
        } else if (type == "reload-config") {
            HyprlandAPI::reloadConfig();
        } else if (type == "toggle-floating") {
            result = callDispatcher("togglefloating", "");
        } else if (type == "toggle-fullscreen") {
            result = callDispatcher("fullscreen", "1");
        } else if (type == "swap-focused-window") {
            result = callDispatcher("swapwindow", action["direction"].asString());
        } else if (type == "swap-direction") {
            result = callDispatcher("swapwindow", action["direction"].asString());
        } else if (type == "move-direction") {
            result = callDispatcher("moveactive", action["direction"].asString());
        } else if (type == "resize-direction") {
            result = callDispatcher("resizeactive", action["direction"].asString());
        } else if (type == "resize-tiled-direction") {
            result = callDispatcher("resizewindow", action["direction"].asString());
        } else if (type == "set-layout") {
            result = callDispatcher("layoutmsg", "layout " + action["name"].asString());
        } else if (type == "cycle-layout") {
            result = callDispatcher(
                "layoutmsg",
                action["direction"].asString() == "previous" ? "cycleprev" : "cyclenext"
            );
        }

        if (!result.success) {
            return result;
        }
    }

    return {};
}

SDispatchResult focusWindowByAddress(const std::string& address) {
    if (address.empty()) {
        return {.passEvent = false, .success = false, .error = "empty focus target"};
    }

    return callDispatcher("focuswindow", "address:" + address);
}

SDispatchResult swapWindowByAddress(const std::string& address) {
    if (address.empty()) {
        return {.passEvent = false, .success = false, .error = "empty swap target"};
    }

    return callDispatcher("swapwindow", "address:" + address);
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
            logJson("command", g_runtime->sendCommand(R"({"type":"reload-config"})"));
        }
        loadLayoutRuntimeConfig();
        Layout::Supplementary::algoMatcher()->updateWorkspaceLayouts();
        resyncAll();
        flushPendingWorkspaceRecalculations();
    }));
}

SDispatchResult hypreactDispatcher(std::string arg) {
    if (!g_runtime) {
        return {.passEvent = false, .success = false, .error = "runtime not initialized"};
    }

    if (arg == "resync") {
        resyncAll();
        return {};
    }

    const auto responseText = g_runtime->sendCommand(arg);
    logJson("command", responseText);
    const auto response = parseJson(responseText);
    if (!response.has_value()) {
        return {.passEvent = false, .success = false, .error = "invalid ffi response"};
    }
    if (!(*response)["ok"].asBool()) {
        return {.passEvent = false, .success = false, .error = (*response)["error"].asString()};
    }

    return applyActions(*response);
}

SDispatchResult interceptDispatcher(const std::string& name, std::string arg) {
    if (!g_runtime) {
        return callDispatcher(name, arg);
    }

    if (name == "movefocus") {
        const auto direction = normalizeDirection(arg);
        if (direction.has_value()) {
            const auto focusResponseText = g_runtime->layoutFocusCandidate(*direction);
            logJson("layout-focus", focusResponseText);
            const auto focusResponse = parseJson(focusResponseText);
            if (focusResponse.has_value() && (*focusResponse)["ok"].asBool()) {
                const auto& target = (*focusResponse)["data"];
                if (target.isString() && !target.asString().empty()) {
                    return focusWindowByAddress(target.asString());
                }
            }
        }
    }

    if (name == "swapwindow") {
        const auto direction = normalizeDirection(arg);
        if (direction.has_value()) {
            const auto swapResponseText = g_runtime->layoutSwapCandidate(*direction);
            logJson("layout-swap", swapResponseText);
            const auto swapResponse = parseJson(swapResponseText);
            if (swapResponse.has_value() && (*swapResponse)["ok"].asBool()) {
                const auto& target = (*swapResponse)["data"];
                if (target.isString() && !target.asString().empty()) {
                    return swapWindowByAddress(target.asString());
                }
            }
        }
    }

    const auto command = commandFromDispatcher(name, arg);
    if (!command.has_value()) {
        if (nativeFallbackEnabled()) {
            return callDispatcher(name, arg);
        }
        return {.passEvent = false, .success = false, .error = "unsupported hypreact translation for dispatcher: " + name};
    }

    const auto responseText = g_runtime->sendCommand(stringify(*command));
    logJson(name.c_str(), responseText);
    const auto response = parseJson(responseText);
    if (!response.has_value()) {
        return {.passEvent = false, .success = false, .error = "invalid ffi response"};
    }
    if (!(*response)["ok"].asBool()) {
        return {.passEvent = false, .success = false, .error = (*response)["error"].asString()};
    }

    return applyActions(*response);
}

void installDispatcherWrappers() {
    for (const auto& name : configuredDispatcherNames()) {
        const auto it = g_pKeybindManager->m_dispatchers.find(name);
        if (it == g_pKeybindManager->m_dispatchers.end()) {
            continue;
        }

        g_originalDispatchers.emplace(name, it->second);
        g_pKeybindManager->m_dispatchers[name] = [name](std::string arg) {
            return interceptDispatcher(name, std::move(arg));
        };
    }
}

void restoreDispatcherWrappers() {
    for (auto& [name, dispatcher] : g_originalDispatchers) {
        g_pKeybindManager->m_dispatchers[name] = dispatcher;
    }

    g_originalDispatchers.clear();
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
        return g_runtime->layoutStatus();
    }

    if (command == "reload-layouts") {
        loadLayoutRuntimeConfig();
        return g_runtime->reloadLayoutConfig();
    }

    Json::Value response;
    response["ok"] = true;

    if (const auto runtimeJson = parseJson(g_runtime->state()); runtimeJson.has_value()) {
        response["data"]["runtime"] = (*runtimeJson)["data"];
    }
    loadLayoutRuntimeConfig();
    if (const auto layoutJson = parseJson(g_runtime->layoutStatus()); layoutJson.has_value()) {
        response["data"]["layouts"] = (*layoutJson)["data"];
    }

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

    HyprlandAPI::addConfigValue(PHANDLE, "plugin:hypreact:enabled_dispatchers", Hyprlang::CConfigValue("exec,movefocus,swapwindow,workspace,movetoworkspace,movetoworkspacesilent,togglefloating,fullscreen,killactive,cyclenext,layoutmsg"));
    HyprlandAPI::addConfigValue(PHANDLE, "plugin:hypreact:fallback_native", Hyprlang::CConfigValue(static_cast<Hyprlang::INT>(1)));
    HyprlandAPI::addConfigValue(PHANDLE, "plugin:hypreact:config_path", Hyprlang::CConfigValue(""));
    g_enabledDispatchersConfig = HyprlandAPI::getConfigValue(PHANDLE, "plugin:hypreact:enabled_dispatchers");
    g_fallbackNativeConfig = HyprlandAPI::getConfigValue(PHANDLE, "plugin:hypreact:fallback_native");
    g_configPathConfig = HyprlandAPI::getConfigValue(PHANDLE, "plugin:hypreact:config_path");

    g_runtime = std::make_unique<Runtime>();
    resyncAll();
    loadLayoutRuntimeConfig();
    registerHypreactAlgorithm();
    registerHooks();
    installDispatcherWrappers();

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
        {"text", std::string{"hypreact loaded with Hyprland dispatcher compatibility"}},
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

    restoreDispatcherWrappers();
    g_listeners.clear();
    g_pendingWorkspaceRecalculations.clear();
    g_pendingWorkspaceLayoutRefreshTicks = 0;
    g_windowIds.clear();
    g_enabledDispatchersConfig = nullptr;
    g_fallbackNativeConfig = nullptr;
    g_configPathConfig = nullptr;
    unregisterHypreactAlgorithm();
    g_runtime.reset();
    PHANDLE = nullptr;
}

#ifdef __clang__
#pragma clang diagnostic pop
#endif
