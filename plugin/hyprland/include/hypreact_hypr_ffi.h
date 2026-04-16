#pragma once

extern "C" {

struct HypreactRuntimeHandle;

struct HypreactWindowSync {
    const char* window_id;
    const char* workspace_id;
    const char* output_id;
    bool is_xwayland;
    bool mapped;
    const char* title;
    const char* app_id;
    const char* class_name;
    const char* instance;
    const char* role;
    const char* window_type;
    bool urgent;
    bool floating;
    bool fullscreen;
};

struct HypreactOutputSync {
    const char* output_id;
    const char* name;
    unsigned int logical_width;
    unsigned int logical_height;
};

struct HypreactWorkspaceLayoutSpaceSync {
    const char* workspace_id;
    const char* output_id;
    int x;
    int y;
    unsigned int width;
    unsigned int height;
};

struct HypreactPlacementGeometry {
    const char* window_id;
    int x;
    int y;
    int width;
    int height;
};

struct HypreactPlacementResult {
    HypreactPlacementGeometry* geometries;
    unsigned long geometry_count;
};

struct HypreactStringResult {
    char* value;
};

enum HypreactDirection {
    HYPREACT_DIRECTION_LEFT = 0,
    HYPREACT_DIRECTION_RIGHT = 1,
    HYPREACT_DIRECTION_UP = 2,
    HYPREACT_DIRECTION_DOWN = 3,
};

enum HypreactLayoutCycleDirection {
    HYPREACT_LAYOUT_CYCLE_NEXT = 0,
    HYPREACT_LAYOUT_CYCLE_PREVIOUS = 1,
};

enum HypreactCommandKind {
    HYPREACT_COMMAND_SPAWN = 0,
    HYPREACT_COMMAND_RELOAD_CONFIG = 1,
    HYPREACT_COMMAND_SET_LAYOUT = 2,
    HYPREACT_COMMAND_CYCLE_LAYOUT = 3,
    HYPREACT_COMMAND_VIEW_WORKSPACE = 4,
    HYPREACT_COMMAND_ACTIVATE_WORKSPACE = 5,
    HYPREACT_COMMAND_TOGGLE_FLOATING = 6,
    HYPREACT_COMMAND_TOGGLE_FULLSCREEN = 7,
    HYPREACT_COMMAND_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 8,
    HYPREACT_COMMAND_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 9,
    HYPREACT_COMMAND_FOCUS_WINDOW = 10,
    HYPREACT_COMMAND_FOCUS_DIRECTION = 11,
    HYPREACT_COMMAND_SWAP_DIRECTION = 12,
    HYPREACT_COMMAND_RESIZE_DIRECTION = 13,
    HYPREACT_COMMAND_MOVE_DIRECTION = 14,
    HYPREACT_COMMAND_FOCUS_NEXT_WINDOW = 15,
    HYPREACT_COMMAND_FOCUS_PREVIOUS_WINDOW = 16,
    HYPREACT_COMMAND_SELECT_NEXT_WORKSPACE = 17,
    HYPREACT_COMMAND_SELECT_PREVIOUS_WORKSPACE = 18,
    HYPREACT_COMMAND_SELECT_WORKSPACE = 19,
    HYPREACT_COMMAND_CLOSE_FOCUSED_WINDOW = 20,
};

struct HypreactCommandInput {
    HypreactCommandKind kind;
    const char* string_value;
    unsigned char workspace;
    HypreactDirection direction;
    HypreactLayoutCycleDirection cycle_direction;
    bool has_cycle_direction;
};

enum HypreactActionKind {
    HYPREACT_ACTION_SPAWN_COMMAND = 0,
    HYPREACT_ACTION_RELOAD_CONFIG = 1,
    HYPREACT_ACTION_SET_LAYOUT = 2,
    HYPREACT_ACTION_CYCLE_LAYOUT = 3,
    HYPREACT_ACTION_ACTIVATE_WORKSPACE = 4,
    HYPREACT_ACTION_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 5,
    HYPREACT_ACTION_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 6,
    HYPREACT_ACTION_TOGGLE_FLOATING = 7,
    HYPREACT_ACTION_TOGGLE_FULLSCREEN = 8,
    HYPREACT_ACTION_FOCUS_WINDOW = 9,
    HYPREACT_ACTION_FOCUS_DIRECTION = 10,
    HYPREACT_ACTION_FOCUS_NEXT_WINDOW = 11,
    HYPREACT_ACTION_FOCUS_PREVIOUS_WINDOW = 12,
    HYPREACT_ACTION_SWAP_DIRECTION = 13,
    HYPREACT_ACTION_MOVE_DIRECTION = 14,
    HYPREACT_ACTION_RESIZE_DIRECTION = 15,
    HYPREACT_ACTION_CLOSE_FOCUSED_WINDOW = 16,
};

struct HypreactAction {
    HypreactActionKind kind;
    char* string_value;
    unsigned char workspace;
    HypreactDirection direction;
    HypreactLayoutCycleDirection cycle_direction;
    bool has_cycle_direction;
};

struct HypreactActionResult {
    HypreactAction* actions;
    unsigned long action_count;
    char* error;
};

struct HypreactStateResult {
    char** workspace_names;
    unsigned long workspace_name_count;
    char* current_workspace_id;
    char* current_output_id;
    char* focused_window_id;
};

struct HypreactDiagnosticRange {
    unsigned int start_line;
    unsigned int start_column;
    unsigned int end_line;
    unsigned int end_column;
};

struct HypreactDiagnostic {
    char* source;
    char* severity;
    char* code;
    char* message;
    char* path;
    HypreactDiagnosticRange range;
};

struct HypreactLayoutStatusResult {
    bool loaded;
    char* config_path;
    char* selected_layout_name;
    char* error;
    HypreactDiagnostic* diagnostics;
    unsigned long diagnostic_count;
    char** workspace_names;
    unsigned long workspace_name_count;
};

struct HypreactStatusResult {
    bool changed;
    char* focused_window_id;
    char* error;
};

HypreactRuntimeHandle* hypreact_runtime_new();
void hypreact_runtime_free(HypreactRuntimeHandle* handle);

HypreactActionResult hypreact_runtime_dispatch_command(HypreactRuntimeHandle* handle, const HypreactCommandInput* command);
HypreactStatusResult hypreact_runtime_reset_state_result(HypreactRuntimeHandle* handle);
HypreactStatusResult hypreact_runtime_upsert_output_result(
    HypreactRuntimeHandle* handle,
    const HypreactOutputSync* output
);
HypreactStatusResult hypreact_runtime_remove_output_result(HypreactRuntimeHandle* handle, const char* output_id);
HypreactStatusResult hypreact_runtime_activate_workspace_result(
    HypreactRuntimeHandle* handle,
    const char* workspace_id,
    const char* output_id
);
HypreactStatusResult hypreact_runtime_set_workspace_layout_space_result(
    HypreactRuntimeHandle* handle,
    const HypreactWorkspaceLayoutSpaceSync* layout_space
);
HypreactStatusResult hypreact_runtime_focus_window_result(
    HypreactRuntimeHandle* handle,
    const char* window_id
);
HypreactStatusResult hypreact_runtime_set_window_closing_result(
    HypreactRuntimeHandle* handle,
    const char* window_id,
    bool closing
);
HypreactStatusResult hypreact_runtime_remove_window_result(HypreactRuntimeHandle* handle, const char* window_id);
HypreactStatusResult hypreact_runtime_upsert_window_result(
    HypreactRuntimeHandle* handle,
    const HypreactWindowSync* window
);
HypreactStatusResult hypreact_runtime_load_layout_config_result(HypreactRuntimeHandle* handle, const char* config_path);
HypreactStatusResult hypreact_runtime_bootstrap_config_result(const char* config_root);
HypreactStatusResult hypreact_runtime_sync_sdk_support_result(const char* config_root);
HypreactStatusResult hypreact_runtime_reload_layout_config_result(HypreactRuntimeHandle* handle);
HypreactStatusResult hypreact_runtime_poll_layout_sources_result(HypreactRuntimeHandle* handle);
int hypreact_runtime_layout_source_change_fd(HypreactRuntimeHandle* handle);
HypreactPlacementResult hypreact_runtime_layout_placement(HypreactRuntimeHandle* handle);
HypreactPlacementResult hypreact_runtime_layout_placement_for_workspace(
    HypreactRuntimeHandle* handle,
    const char* workspace_id
);
HypreactStringResult hypreact_runtime_layout_focus_candidate(HypreactRuntimeHandle* handle, const char* direction);
HypreactStringResult hypreact_runtime_layout_close_focus_candidate(HypreactRuntimeHandle* handle, const char* window_id);
HypreactStringResult hypreact_runtime_layout_swap_candidate(HypreactRuntimeHandle* handle, const char* direction);
HypreactStatusResult hypreact_runtime_resize_direction(HypreactRuntimeHandle* handle, const char* direction);
HypreactStatusResult hypreact_runtime_move_tiled_window(
    HypreactRuntimeHandle* handle,
    const char* first_window_id,
    const char* second_window_id
);
HypreactStateResult hypreact_runtime_state_result(HypreactRuntimeHandle* handle);
HypreactLayoutStatusResult hypreact_runtime_layout_status_result(HypreactRuntimeHandle* handle);

void hypreact_string_free(char* value);
void hypreact_runtime_free_placement_result(HypreactPlacementResult result);
void hypreact_runtime_free_action_result(HypreactActionResult result);
void hypreact_runtime_free_state_result(HypreactStateResult result);
void hypreact_runtime_free_layout_status_result(HypreactLayoutStatusResult result);
void hypreact_runtime_free_status_result(HypreactStatusResult result);

}
