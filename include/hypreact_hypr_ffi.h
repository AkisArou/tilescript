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

HypreactRuntimeHandle* hypreact_runtime_new();
void hypreact_runtime_free(HypreactRuntimeHandle* handle);

char* hypreact_runtime_handle_command(HypreactRuntimeHandle* handle, const char* command_json);
char* hypreact_runtime_reset_state(HypreactRuntimeHandle* handle);
char* hypreact_runtime_upsert_output(
    HypreactRuntimeHandle* handle,
    const HypreactOutputSync* output
);
char* hypreact_runtime_remove_output(HypreactRuntimeHandle* handle, const char* output_id);
char* hypreact_runtime_activate_workspace(
    HypreactRuntimeHandle* handle,
    const char* workspace_id,
    const char* output_id
);
char* hypreact_runtime_focus_window(
    HypreactRuntimeHandle* handle,
    const char* window_id
);
char* hypreact_runtime_remove_window(HypreactRuntimeHandle* handle, const char* window_id);
char* hypreact_runtime_upsert_window(
    HypreactRuntimeHandle* handle,
    const HypreactWindowSync* window
);
char* hypreact_runtime_load_layout_config(HypreactRuntimeHandle* handle, const char* config_path);
char* hypreact_runtime_reload_layout_config(HypreactRuntimeHandle* handle);
char* hypreact_runtime_layout_status(HypreactRuntimeHandle* handle);
char* hypreact_runtime_layout_placement(HypreactRuntimeHandle* handle);
char* hypreact_runtime_layout_focus_candidate(HypreactRuntimeHandle* handle, const char* direction);
char* hypreact_runtime_layout_swap_candidate(HypreactRuntimeHandle* handle, const char* direction);
char* hypreact_runtime_layout_resize_master(HypreactRuntimeHandle* handle, double delta);
char* hypreact_runtime_state(HypreactRuntimeHandle* handle);

void hypreact_string_free(char* value);

}
