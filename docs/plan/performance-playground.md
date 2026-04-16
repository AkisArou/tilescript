# Playground Performance Plan

## Goals

- Keep the playground responsive while editing, switching focus, and interacting with the preview.
- Make the embedded preview editor stable enough that Monaco does not visibly reinitialize on ordinary preview updates.
- Reduce redundant config/runtime work so the browser path matches the artifact-driven architecture more closely.
- Improve maintainability by separating long-lived UI/editor state from volatile preview scene state.

## Current Findings

### 1. Config work is duplicated

The playground currently had two async pipelines over the same source bundle inputs:

- config loading
- preview evaluation

Before the first cleanup pass, preview evaluation reloaded config again even though the app already had a loaded config. This doubled work on every edit and made the async flow harder to reason about.

Status:

- fixed in the first pass by making preview evaluation consume the already-loaded config

### 2. Preview rendering is too broad and too volatile

The Preview tab currently depends on `app_state.session.get()` in many nested closures. Small state changes such as focus updates can fan out through:

- top bar labels
- preview scene availability checks
- window frame styles
- focused window indicators
- window list rendering

This is especially problematic for expensive embedded subtrees.

Status:

- partially improved by keying preview windows by `window.id`
- still needs deeper narrowing of reactive dependencies

### 3. Embedded editor stability is structurally fragile

The preview editor window lives inside the preview scene window list. Even with keyed rendering, it is still conceptually coupled to scene-window updates, which makes Monaco/editor stability dependent on broader preview rerender behavior.

This is the main reason the preview editor can flash or feel unstable under focus changes.

Status:

- partially improved by keyed preview window rendering
- not fully solved yet

### 4. Config reload and preview reevaluation triggers are entangled

The app mixes two concepts:

- source/config changes that require reparsing config
- preview-only changes that only require reevaluating preview geometry/scene

Examples of preview-only changes:

- focus changes
- window movement
- resize actions
- workspace selection inside the simulated WM

Examples of config changes:

- editing source files
- changing authoring language
- creating layouts

These paths should stay distinct.

Status:

- improved in the first pass by replacing the config request hash with an explicit `latest_config_request_id`
- still needs cleaner ownership of triggers and fewer broad signal reads

Additional concrete issue found during startup stabilization:

- successful preview application was writing `loaded_config` again even though config loading had already produced it
- the preview effect depends on `loaded_config`, so preview completion could retrigger preview evaluation and create a feedback loop on startup

Status:

- fixed by making `loaded_config` authoritative only for config load completion, not preview completion

### 5. Monaco/editor lifecycle is local-component scoped

`MonacoEditorPane` stores the editor handle inside the component instance. That is fine when the component is mounted in a stable shell, but it becomes fragile when mounted inside volatile parents.

There was also a concrete host-level issue in the dual-editor setup: Monaco text models are global, but the playground host was disposing them as if they were owned by a single editor instance. With two live Monaco panes, one pane lifecycle could tear down shared models and force syntax/token state to restart in the other pane.

Longer-term, embedded-editor stability may require either:

- a more stable parent shell outside the volatile preview scene loop
- or an editor-host abstraction that survives subtree churn better

Status:

- improved by switching the Monaco host to shared model ref-counting instead of unconditional model disposal on editor teardown

## First Pass Completed

The following safe improvements have already been made:

1. Removed duplicate config load during preview evaluation.
2. Switched config loading from a buffer-debug-string request key to an explicit request id.
3. Restored full `EditorView` in the preview editor window.
4. Keyed preview windows by `window.id` to reduce remount churn for stable windows.

## Phase 1 Progress

Additional progress completed after the first pass:

1. Moved the embedded preview editor onto a stable shell outside the normal preview window list while still using the fake preview window geometry from the scene.
2. Split preview scene rendering into smaller units:
   - `PreviewSceneSurface`
   - `PreviewWindowFrame`
   - `StablePreviewEditorWindow`
3. Derived per-window render state up front so normal preview windows no longer repeatedly query the whole session and scene inside nested closures.
4. Separated source-change config reloads from preview-only reevaluation triggers so the hot edit path no longer schedules duplicate preview passes before and after config load.
5. Split `PreviewView` chrome and surface routing into smaller components so toolbar/session reads are no longer embedded in the same large render block as the preview scene.
6. Switched the Monaco host to shared model ownership so dual live editor panes no longer dispose each other’s Monaco models during pane lifecycle changes.
7. Removed a preview-startup feedback loop by preventing successful preview application from rewriting `loaded_config`.

This does not finish Phase 1, but it establishes the right structure for the remaining work.

## Next Phases

### Phase 1: Stabilize preview/editor boundaries

- Split preview window rendering into smaller components with narrower signals.
- Move the embedded editor window onto a more stable shell so focus/frame updates do not threaten the editor subtree.
- Ensure preview interaction handlers do not interfere with editor pointer/keyboard focus.

Exit criteria:

- focusing other preview windows does not visibly flash Monaco
- syntax highlighting does not visibly reinitialize on ordinary focus changes

### Phase 2: Narrow reactive dependencies

- Replace repeated `app_state.session.get()` reads in nested closures with smaller derived signals or per-window props.
- Avoid cloning/reading whole session state in places that only need:
  - focused window id
  - workspace names
  - current scene existence
  - diagnostics count
- Separate top-bar reactive state from scene-window rendering state.

Exit criteria:

- preview frame/focus changes do not rerender unrelated editor chrome
- code paths are easier to inspect and reason about

### Phase 3: Clean async pipeline ownership

- Make config loading and preview evaluation more explicitly staged:
  - source change -> config load
  - config + preview state -> preview evaluation
- Reduce duplicated source-bundle reconstruction where practical.
- Consider caching derived source bundle inputs or fingerprints if needed after measurement.

Exit criteria:

- async flow is single-responsibility and easy to trace
- no duplicate config parse/evaluate work on the hot edit path

### Phase 4: Maintainability cleanup

- Break large preview view logic into focused components:
  - preview chrome/top bar
  - preview scene surface
  - preview window frame
  - embedded editor window shell
- Keep cross-cutting helpers close to their owning feature.
- Add a small amount of documentation around which state is authoritative for:
  - editor content
  - loaded config
  - preview session/model
  - evaluated preview scene

## Constraints

- Do not regress the existing Preview / Editor / Diagnostics / Binds workflow.
- Keep the standalone Editor tab.
- Preserve the fake window simulation model for the preview.
- Prefer structural fixes over local hacks inside Monaco glue code.
- Prefer browser-native transitions/keyframes over custom animation engines or Hyprland-specific animation logic.

## Diagnostics

Before changing preview window animations further, add lightweight instrumentation so we can distinguish:

- DOM remounts
- style-only updates
- geometry changes
- expensive async reevaluations

Recommended checks:

1. Add temporary mount/unmount logs for preview window components, especially the stable preview editor shell and normal preview window frames.
2. Log focus-change and workspace-change request flow:
   - command applied
   - preview reevaluation requested
   - preview reevaluation completed
3. Use browser DevTools and MCP snapshots/tracing to confirm whether focus change causes:
   - actual node replacement
   - repeated layout/style recalculation
   - repeated Monaco mount/dispose
4. Keep instrumentation cheap and easy to remove once the behavior is understood.

This should be used to answer two questions before any animation changes:

- are windows really being recreated on focus change?
- if not, which browser-native transition is producing the perceived animation?

## Immediate Next Recommended Change

Implement a stable embedded preview-editor shell that is not recreated as part of ordinary preview window list churn, while still visually behaving like a preview window.
