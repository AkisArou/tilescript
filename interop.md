# Interop Plan

This document describes how to make the Rust <-> C++ boundary in `hypreact` as typed and maintainable as possible.

Scope:

- Rust FFI crate: `crates/hypr-ffi`
- Hyprland plugin bridge: `plugin/hyprland/`

Non-goals:

- changing user-facing `hyprctl hypreact` JSON output
- redesigning Hyprland plugin behavior
- replacing the C ABI with C++ bindings or code generation right now

## Goals

- typed plugin/runtime protocol
- minimal stringly-typed data crossing FFI
- stable ownership and free rules
- fewer JSON parse/stringify steps inside plugin internals
- clear separation between:
  - internal plugin/runtime protocol
  - user-facing inspection/debug output

## Current Audit

## Already Typed And In Good Shape

These FFI surfaces are already structured and should stay that way.

Input structs:

- `HypreactWindowSync`
- `HypreactOutputSync`
- `HypreactWorkspaceLayoutSpaceSync`
- `HypreactCommandInput`

Result structs:

- `HypreactActionResult`
- `HypreactStatusResult`
- `HypreactPlacementResult`
- `HypreactStateResult`
- `HypreactLayoutStatusResult`
- `HypreactStringResult`

Typed operational calls already in good shape:

- `hypreact_runtime_dispatch_command`
- `hypreact_runtime_load_layout_config_result`
- `hypreact_runtime_bootstrap_config_result`
- `hypreact_runtime_sync_sdk_support_result`
- `hypreact_runtime_reload_layout_config_result`
- `hypreact_runtime_layout_placement`
- `hypreact_runtime_layout_placement_for_workspace`
- `hypreact_runtime_layout_focus_candidate`
- `hypreact_runtime_layout_close_focus_candidate`
- `hypreact_runtime_layout_swap_candidate`
- `hypreact_runtime_resize_direction`
- `hypreact_runtime_move_tiled_window`
- `hypreact_runtime_state_result`
- `hypreact_runtime_layout_status_result`

These are the model for the rest of the boundary.

## Still JSON/String Based Internally

These are the remaining internal FFI calls that still return JSON strings instead of typed status structs:

- `hypreact_runtime_reset_state`
- `hypreact_runtime_upsert_output`
- `hypreact_runtime_remove_output`
- `hypreact_runtime_activate_workspace`
- `hypreact_runtime_set_workspace_layout_space`
- `hypreact_runtime_focus_window`
- `hypreact_runtime_set_window_closing`
- `hypreact_runtime_remove_window`
- `hypreact_runtime_upsert_window`
- `hypreact_runtime_state`

Current plugin bridge usage:

- `plugin/hyprland/src/plugin_runtime.cpp` wraps these JSON-returning calls in `Runtime::take(...)`
- `plugin/hyprland/src/plugin.cpp` mostly just logs those JSON strings through `logJson("sync", ...)`
- plugin behavior itself usually only cares about a tiny typed subset:
  - changed or unchanged
  - maybe focused window id
  - maybe error

This is the main remaining internal cleanup target.

## JSON That Should Remain JSON

These surfaces should stay JSON, because they are user-facing or intentionally open-ended.

### `hyprctl hypreact`

Keep this JSON.

Why:

- it is a user-facing inspection/debug interface
- it benefits from being extensible
- it naturally wants nested structured data
- exact field shape can evolve more easily than a typed C ABI

### Diagnostics payload in `hyprctl hypreact`

Keep this JSON in the `hyprctl` response.

The plugin may still receive structured diagnostics from FFI, but the `hyprctl` output itself should remain JSON.

## Problems In The Current Boundary

## 1. Internal sync path still uses JSON strings

This is the biggest remaining design debt.

Example pattern today:

- Rust mutates model state
- Rust serializes `StatusResult` to JSON text
- C++ receives `char*`
- C++ wraps it into `std::string`
- C++ logs the JSON text
- C++ usually does not inspect typed fields

Problems:

- no compile-time schema between Rust and C++
- unnecessary serialization/deserialization mindset even when C++ only needs `changed`
- encourages "just put more fields in JSON" instead of designing the ABI
- harder to reason about ownership and semantics than direct typed returns

## 2. FFI type layering needed cleanup

This was previously mixed together in one file, but has now been split into:

- `crates/hypr-ffi/src/abi.rs`
  - `#[repr(C)]` ABI structs and enums only
- `crates/hypr-ffi/src/runtime_types.rs`
  - Rust-only runtime helper types

That makes the boundary easier to audit and keeps Rust-only transport helpers out of the ABI module.

## 3. Diagnostics were previously stringly typed

This used to be carried as `diagnostics_json` inside `HypreactLayoutStatusResult`.

That is no longer the case. Diagnostics now cross the boundary as typed arrays and are only rendered to JSON when building `hyprctl hypreact` output.

## Desired End State

## Boundary Rule

Use typed C ABI structs for plugin/runtime protocol.

Use JSON only for:

- `hyprctl hypreact`
- optional debug dumps that are explicitly user-facing

Everything else between `plugin.cpp`/`plugin_runtime.cpp` and `crates/hypr-ffi` should be typed.

## Ownership Rule

Prefer one of these patterns only:

1. plain scalar/status return values
2. struct returns with explicit free function
3. result arrays with explicit count + free function

Avoid raw JSON string payloads for internal protocol.

## Layering Rule

- Rust `crates/hypr-ffi` owns C ABI types and conversion
- C++ `plugin_runtime.cpp` is a thin typed adapter over the FFI
- `plugin.cpp` should not know or care about Rust JSON payload internals

## Concrete Migration Plan

## Stage 1. Replace JSON sync calls with typed status results

Replace these FFI functions:

- `hypreact_runtime_reset_state`
- `hypreact_runtime_upsert_output`
- `hypreact_runtime_remove_output`
- `hypreact_runtime_activate_workspace`
- `hypreact_runtime_set_workspace_layout_space`
- `hypreact_runtime_focus_window`
- `hypreact_runtime_set_window_closing`
- `hypreact_runtime_remove_window`
- `hypreact_runtime_upsert_window`

With typed `_result` variants returning `HypreactStatusResult`.

Recommended naming:

- `hypreact_runtime_reset_state_result`
- `hypreact_runtime_upsert_output_result`
- `hypreact_runtime_remove_output_result`
- `hypreact_runtime_activate_workspace_result`
- `hypreact_runtime_set_workspace_layout_space_result`
- `hypreact_runtime_focus_window_result`
- `hypreact_runtime_set_window_closing_result`
- `hypreact_runtime_remove_window_result`
- `hypreact_runtime_upsert_window_result`

Why `_result`:

- consistent with the already-cleaner typed APIs
- easy to migrate incrementally

Implementation note:

- keep the old JSON functions briefly only if necessary during migration
- otherwise remove them directly if plugin code is updated in the same patch

## Stage 2. Extend `HypreactStatusResult` where needed

Current `HypreactStatusResult`:

- `changed`
- `error`

This is close, but internal sync calls may also need:

- `focused_window_id`

That field already exists in Rust-side `StatusResult`, but not in the C ABI struct.

Recommended shape:

```c
struct HypreactStatusResult {
    bool changed;
    char* focused_window_id;
    char* error;
};
```

Then:

- `remove_window_result` can return the preferred next focus candidate directly
- plugin code can react without parsing JSON or inventing side channels

This is the first structural FFI change I would make.

## Stage 3. Remove `Runtime::take(...)` from `plugin_runtime.cpp`

After Stage 1, these methods should stop returning `std::string` JSON payloads:

- `Runtime::resetState`
- `Runtime::upsertOutput`
- `Runtime::removeOutput`
- `Runtime::activateWorkspace`
- `Runtime::setWorkspaceLayoutSpace`
- `Runtime::focusWindow`
- `Runtime::setWindowClosing`
- `Runtime::removeWindow`
- `Runtime::upsertWindow`

They should instead return `HypreactStatusResult`.

Then remove:

- `Runtime::take(...)`

This is a very strong cleanup milestone because it removes the main internal JSON hack from the bridge.

## Stage 4. Replace `logSyncResponse(std::string)` with typed logging

Current plugin flow logs raw JSON strings for sync calls.

Replace with something like:

```cpp
void logStatusResult(const char* label, const HypreactStatusResult& result);
```

Then sync sites can do:

- call typed FFI
- log typed result
- free typed result

This keeps debugging useful without making JSON the transport.

## Stage 5. Split Rust ABI structs from Rust-only helpers

Completed.

Current file shape:

- `abi.rs`
  - `#[repr(C)]` types and enums only
- `runtime_types.rs`
  - internal Rust helper types

## Stage 6. Replace `diagnostics_json` with typed diagnostics arrays

Completed.

Current status:

- `HypreactLayoutStatusResult` contains typed diagnostics entries and a count

Recommended end state:

```c
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

```

Implemented as embedded diagnostics in `HypreactLayoutStatusResult`.

Why this shape works:

- diagnostics are part of runtime status
- plugin notifications and `hyprctl` status consume the same typed data
- only the final `hyprctl` response is JSON

## Stage 7. Remove `hypreact_runtime_state(...)` JSON function

This one is easy cleanup.

Current:

- both `hypreact_runtime_state(...)` JSON and `hypreact_runtime_state_result(...)` typed exist

Plan:

- remove `hypreact_runtime_state(...)`
- keep only `hypreact_runtime_state_result(...)`

This is low-risk and should happen early.

## Stage 8. Keep command dispatch typed

`hypreact_runtime_dispatch_command(...)` is already on the right track.

Keep:

- typed command input enum + payload fields
- typed action result array

Potential future improvement:

- consider splitting `HypreactCommandInput` into more explicit tagged payload structs if it grows significantly

But this is not urgent.

## Proposed Migration Order

Do this in order:

1. add `focused_window_id` to C ABI `HypreactStatusResult`
2. convert all internal JSON sync calls to typed `_result` APIs
3. update `plugin_runtime.cpp` to use typed status results
4. remove `Runtime::take(...)`
5. remove `hypreact_runtime_state(...)` JSON API
6. split FFI ABI types from Rust-only helper structs
7. replace `diagnostics_json` with typed diagnostics arrays

Current status:

- stages 1 through 7 are complete

This order gives the biggest cleanliness win early without forcing a huge one-shot ABI rewrite.

## Concrete File-Level Plan

## `plugin/hyprland/include/hypreact_hypr_ffi.h`

Current state:

- `HypreactStatusResult` includes `focused_window_id`
- typed diagnostics structs are present
- old JSON-returning internal declarations are removed

## `crates/hypr-ffi/src/lib.rs`

Current state:

- typed `_result` variants are in place for sync/update calls
- JSON string wrappers for internal state sync are removed
- layout status frees typed diagnostics arrays explicitly

## `plugin/hyprland/src/plugin_runtime.cpp`

Current state:

- sync methods use typed status returns
- `Runtime::take(...)` is gone
- diagnostics notifications consume typed diagnostics directly

## `plugin/hyprland/src/plugin.cpp`

Current state:

- internal sync is not treated as JSON text
- JSON is built only for `hyprctl hypreact`
- `hyprctl` diagnostics JSON is derived from typed diagnostics

## `crates/hypr-ffi/src/abi.rs` and `crates/hypr-ffi/src/runtime_types.rs`

Current state:

- ABI and Rust-only helper types are split

## Acceptance Criteria

The boundary is considered clean when all of these are true:

- no internal plugin/runtime state-sync call returns JSON text
- `Runtime::take(...)` is gone
- `plugin.cpp` only parses/builds JSON for `hyprctl hypreact`
- all operational FFI calls use typed result structs
- diagnostics are typed, not stuffed into `diagnostics_json`
- free rules are explicit and symmetric for every typed allocated result

## What Not To Do

- do not replace typed structs with one giant generic JSON blob API
- do not add more stringly-typed pockets into typed status structs
- do not keep both old JSON API and typed API around indefinitely
- do not introduce heavy codegen or IDL tooling unless the ABI grows much larger than it is today

## Recommendation

The main interop cleanup goals described in this document are now complete.

Further work, if needed, should focus on:

1. keeping new FFI additions in the split `abi.rs` / `runtime_types.rs` shape
2. preserving typed plugin/runtime protocol for all new internal operations
3. keeping JSON confined to `hyprctl hypreact` and other explicitly user-facing inspection output
