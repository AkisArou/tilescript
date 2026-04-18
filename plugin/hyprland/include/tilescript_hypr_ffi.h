#pragma once

extern "C" {

struct TilescriptRuntimeHandle;

struct TilescriptWindowSync {
    const char* window_id;
    const char* previous_focused_window_id;
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

struct TilescriptOutputSync {
    const char* output_id;
    const char* name;
    unsigned int logical_width;
    unsigned int logical_height;
};

struct TilescriptWorkspaceLayoutSpaceSync {
    const char* workspace_id;
    const char* output_id;
    int x;
    int y;
    unsigned int width;
    unsigned int height;
};

struct TilescriptPlacementGeometry {
    const char* window_id;
    int x;
    int y;
    int width;
    int height;
};

struct TilescriptPlacementResult {
    TilescriptPlacementGeometry* geometries;
    unsigned long geometry_count;
};

struct TilescriptStringResult {
    char* value;
};

enum TilescriptDirection {
    TILESCRIPT_DIRECTION_LEFT = 0,
    TILESCRIPT_DIRECTION_RIGHT = 1,
    TILESCRIPT_DIRECTION_UP = 2,
    TILESCRIPT_DIRECTION_DOWN = 3,
};

enum TilescriptLayoutCycleDirection {
    TILESCRIPT_LAYOUT_CYCLE_NEXT = 0,
    TILESCRIPT_LAYOUT_CYCLE_PREVIOUS = 1,
};

enum TilescriptCommandKind {
    TILESCRIPT_COMMAND_SPAWN = 0,
    TILESCRIPT_COMMAND_RELOAD_CONFIG = 1,
    TILESCRIPT_COMMAND_SET_LAYOUT = 2,
    TILESCRIPT_COMMAND_CYCLE_LAYOUT = 3,
    TILESCRIPT_COMMAND_VIEW_WORKSPACE = 4,
    TILESCRIPT_COMMAND_ACTIVATE_WORKSPACE = 5,
    TILESCRIPT_COMMAND_TOGGLE_FLOATING = 6,
    TILESCRIPT_COMMAND_TOGGLE_FULLSCREEN = 7,
    TILESCRIPT_COMMAND_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 8,
    TILESCRIPT_COMMAND_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 9,
    TILESCRIPT_COMMAND_FOCUS_WINDOW = 10,
    TILESCRIPT_COMMAND_FOCUS_DIRECTION = 11,
    TILESCRIPT_COMMAND_SWAP_DIRECTION = 12,
    TILESCRIPT_COMMAND_RESIZE_DIRECTION = 13,
    TILESCRIPT_COMMAND_MOVE_DIRECTION = 14,
    TILESCRIPT_COMMAND_FOCUS_NEXT_WINDOW = 15,
    TILESCRIPT_COMMAND_FOCUS_PREVIOUS_WINDOW = 16,
    TILESCRIPT_COMMAND_SELECT_NEXT_WORKSPACE = 17,
    TILESCRIPT_COMMAND_SELECT_PREVIOUS_WORKSPACE = 18,
    TILESCRIPT_COMMAND_SELECT_WORKSPACE = 19,
    TILESCRIPT_COMMAND_CLOSE_FOCUSED_WINDOW = 20,
};

struct TilescriptCommandInput {
    TilescriptCommandKind kind;
    const char* string_value;
    unsigned char workspace;
    TilescriptDirection direction;
    TilescriptLayoutCycleDirection cycle_direction;
    bool has_cycle_direction;
};

enum TilescriptActionKind {
    TILESCRIPT_ACTION_SPAWN_COMMAND = 0,
    TILESCRIPT_ACTION_RELOAD_CONFIG = 1,
    TILESCRIPT_ACTION_SET_LAYOUT = 2,
    TILESCRIPT_ACTION_CYCLE_LAYOUT = 3,
    TILESCRIPT_ACTION_ACTIVATE_WORKSPACE = 4,
    TILESCRIPT_ACTION_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 5,
    TILESCRIPT_ACTION_TOGGLE_ASSIGN_FOCUSED_WINDOW_TO_WORKSPACE = 6,
    TILESCRIPT_ACTION_TOGGLE_FLOATING = 7,
    TILESCRIPT_ACTION_TOGGLE_FULLSCREEN = 8,
    TILESCRIPT_ACTION_FOCUS_WINDOW = 9,
    TILESCRIPT_ACTION_FOCUS_DIRECTION = 10,
    TILESCRIPT_ACTION_FOCUS_NEXT_WINDOW = 11,
    TILESCRIPT_ACTION_FOCUS_PREVIOUS_WINDOW = 12,
    TILESCRIPT_ACTION_SWAP_DIRECTION = 13,
    TILESCRIPT_ACTION_MOVE_DIRECTION = 14,
    TILESCRIPT_ACTION_RESIZE_DIRECTION = 15,
    TILESCRIPT_ACTION_CLOSE_FOCUSED_WINDOW = 16,
};

struct TilescriptAction {
    TilescriptActionKind kind;
    char* string_value;
    unsigned char workspace;
    TilescriptDirection direction;
    TilescriptLayoutCycleDirection cycle_direction;
    bool has_cycle_direction;
};

struct TilescriptActionResult {
    TilescriptAction* actions;
    unsigned long action_count;
    char* error;
};

struct TilescriptStateResult {
    char** workspace_names;
    unsigned long workspace_name_count;
    char* current_workspace_id;
    char* current_output_id;
    char* focused_window_id;
};

struct TilescriptDiagnosticRange {
    unsigned int start_line;
    unsigned int start_column;
    unsigned int end_line;
    unsigned int end_column;
};

struct TilescriptDiagnostic {
    char* source;
    char* severity;
    char* code;
    char* message;
    char* path;
    TilescriptDiagnosticRange range;
};

struct TilescriptLayoutStatusResult {
    bool loaded;
    char* config_path;
    char* selected_layout_name;
    char* error;
    TilescriptDiagnostic* diagnostics;
    unsigned long diagnostic_count;
    char** workspace_names;
    unsigned long workspace_name_count;
};

struct TilescriptStatusResult {
    bool changed;
    char* focused_window_id;
    char* error;
};

TilescriptRuntimeHandle* tilescript_runtime_new();
void tilescript_runtime_free(TilescriptRuntimeHandle* handle);

TilescriptActionResult tilescript_runtime_dispatch_command(TilescriptRuntimeHandle* handle, const TilescriptCommandInput* command);
TilescriptStatusResult tilescript_runtime_reset_state_result(TilescriptRuntimeHandle* handle);
TilescriptStatusResult tilescript_runtime_upsert_output_result(
    TilescriptRuntimeHandle* handle,
    const TilescriptOutputSync* output
);
TilescriptStatusResult tilescript_runtime_remove_output_result(TilescriptRuntimeHandle* handle, const char* output_id);
TilescriptStatusResult tilescript_runtime_activate_workspace_result(
    TilescriptRuntimeHandle* handle,
    const char* workspace_id,
    const char* output_id
);
TilescriptStatusResult tilescript_runtime_set_workspace_layout_space_result(
    TilescriptRuntimeHandle* handle,
    const TilescriptWorkspaceLayoutSpaceSync* layout_space
);
TilescriptStatusResult tilescript_runtime_focus_window_result(
    TilescriptRuntimeHandle* handle,
    const char* window_id
);
TilescriptStatusResult tilescript_runtime_set_window_closing_result(
    TilescriptRuntimeHandle* handle,
    const char* window_id,
    bool closing
);
TilescriptStatusResult tilescript_runtime_remove_window_result(TilescriptRuntimeHandle* handle, const char* window_id);
TilescriptStatusResult tilescript_runtime_upsert_window_result(
    TilescriptRuntimeHandle* handle,
    const TilescriptWindowSync* window
);
TilescriptStatusResult tilescript_runtime_load_layout_config_result(TilescriptRuntimeHandle* handle, const char* config_path);
TilescriptStatusResult tilescript_runtime_bootstrap_config_result(const char* config_root);
TilescriptStatusResult tilescript_runtime_sync_sdk_support_result(const char* config_root);
TilescriptStatusResult tilescript_runtime_reload_layout_config_result(TilescriptRuntimeHandle* handle);
TilescriptStatusResult tilescript_runtime_poll_layout_sources_result(TilescriptRuntimeHandle* handle);
int tilescript_runtime_layout_source_change_fd(TilescriptRuntimeHandle* handle);
TilescriptPlacementResult tilescript_runtime_layout_placement(TilescriptRuntimeHandle* handle);
TilescriptPlacementResult tilescript_runtime_layout_placement_for_workspace(
    TilescriptRuntimeHandle* handle,
    const char* workspace_id
);
TilescriptStringResult tilescript_runtime_layout_focus_candidate(TilescriptRuntimeHandle* handle, const char* direction);
TilescriptStringResult tilescript_runtime_layout_close_focus_candidate(TilescriptRuntimeHandle* handle, const char* window_id);
TilescriptStringResult tilescript_runtime_layout_swap_candidate(TilescriptRuntimeHandle* handle, const char* direction);
TilescriptStatusResult tilescript_runtime_layout_move_direction(TilescriptRuntimeHandle* handle, const char* direction);
TilescriptStatusResult tilescript_runtime_resize_direction(TilescriptRuntimeHandle* handle, const char* direction);
TilescriptStatusResult tilescript_runtime_move_tiled_window(
    TilescriptRuntimeHandle* handle,
    const char* first_window_id,
    const char* second_window_id
);
TilescriptStateResult tilescript_runtime_state_result(TilescriptRuntimeHandle* handle);
TilescriptLayoutStatusResult tilescript_runtime_layout_status_result(TilescriptRuntimeHandle* handle);

void tilescript_string_free(char* value);
void tilescript_runtime_free_placement_result(TilescriptPlacementResult result);
void tilescript_runtime_free_action_result(TilescriptActionResult result);
void tilescript_runtime_free_state_result(TilescriptStateResult result);
void tilescript_runtime_free_layout_status_result(TilescriptLayoutStatusResult result);
void tilescript_runtime_free_status_result(TilescriptStatusResult result);

}
