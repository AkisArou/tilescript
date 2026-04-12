use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ResolvedLayoutNode;
use crate::focus::{FocusScopeNavigation, FocusScopePath, FocusTree};
use crate::types::LayoutRef;
use crate::{OutputId, WindowId, WorkspaceId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputModel {
    pub id: OutputId,
    pub name: String,
    pub logical_x: i32,
    pub logical_y: i32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub enabled: bool,
    pub focused_workspace_id: Option<WorkspaceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LayoutSpaceBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowModel {
    pub id: WindowId,
    pub is_xwayland: bool,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub class: Option<String>,
    pub instance: Option<String>,
    pub role: Option<String>,
    pub window_type: Option<String>,
    pub output_id: Option<OutputId>,
    pub workspace_id: Option<WorkspaceId>,
    pub mapped: bool,
    pub focused: bool,
    pub floating: bool,
    pub floating_geometry: Option<WindowGeometry>,
    pub fullscreen: bool,
    pub urgent: bool,
    pub closing: bool,
}

impl Default for WindowModel {
    fn default() -> Self {
        Self {
            id: WindowId(String::new()),
            is_xwayland: false,
            app_id: None,
            title: None,
            class: None,
            instance: None,
            role: None,
            window_type: None,
            output_id: None,
            workspace_id: None,
            mapped: false,
            focused: false,
            floating: false,
            floating_geometry: None,
            fullscreen: false,
            urgent: false,
            closing: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceModel {
    pub id: WorkspaceId,
    pub name: String,
    pub output_id: Option<OutputId>,
    pub layout_space: Option<LayoutSpaceBox>,
    pub focused: bool,
    pub visible: bool,
    pub effective_layout: Option<LayoutRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WmModel {
    pub windows: BTreeMap<WindowId, WindowModel>,
    pub workspaces: BTreeMap<WorkspaceId, WorkspaceModel>,
    pub outputs: BTreeMap<OutputId, OutputModel>,
    pub focused_window_id: Option<WindowId>,
    pub current_workspace_id: Option<WorkspaceId>,
    pub current_output_id: Option<OutputId>,
    pub focus_tree: Option<FocusTree>,
    pub last_focused_window_id_by_scope: BTreeMap<FocusScopePath, WindowId>,
    pub tiled_window_order_by_workspace: BTreeMap<WorkspaceId, Vec<WindowId>>,
}

impl WmModel {
    pub fn focused_window_id(&self) -> Option<&WindowId> {
        self.focused_window_id.as_ref()
    }

    pub fn current_workspace_id(&self) -> Option<&WorkspaceId> {
        self.current_workspace_id.as_ref()
    }

    pub fn current_output_id(&self) -> Option<&OutputId> {
        self.current_output_id.as_ref()
    }

    pub fn has_window(&self, id: &WindowId) -> bool {
        self.windows.contains_key(id)
    }

    pub fn workspace_names_for_window(&self, id: &WindowId) -> Vec<String> {
        self.windows
            .get(id)
            .and_then(|window| window.workspace_id.as_ref())
            .and_then(|workspace_id| self.workspaces.get(workspace_id))
            .map(|workspace| vec![workspace.name.clone()])
            .unwrap_or_default()
    }

    pub fn workspace_names(&self) -> Vec<String> {
        self.workspaces.values().map(|workspace| workspace.name.clone()).collect()
    }

    pub fn visible_window_ids(&self) -> Vec<WindowId> {
        self.windows
            .values()
            .filter(|window| {
                window.mapped
                    && self.current_workspace_id.as_ref().is_none_or(|workspace_id| {
                        window.workspace_id.as_ref() == Some(workspace_id)
                    })
            })
            .map(|window| window.id.clone())
            .collect()
    }

    pub fn floating_geometry(&self, id: &WindowId) -> Option<WindowGeometry> {
        self.windows.get(id).and_then(|window| window.floating_geometry)
    }

    pub fn active_workspace_names(&self, workspace: &WorkspaceModel) -> Vec<String> {
        workspace
            .output_id
            .as_ref()
            .map(|output_id| {
                self.workspaces
                    .values()
                    .filter(|candidate| {
                        candidate.visible && candidate.output_id.as_ref() == Some(output_id)
                    })
                    .map(|candidate| candidate.name.clone())
                    .collect()
            })
            .filter(|names: &Vec<String>| !names.is_empty())
            .unwrap_or_else(|| {
                if workspace.visible { vec![workspace.name.clone()] } else { Vec::new() }
            })
    }

    pub fn upsert_workspace(&mut self, workspace_id: WorkspaceId, name: String) {
        self.workspaces
            .entry(workspace_id.clone())
            .and_modify(|workspace| {
                workspace.name = name.clone();
            })
            .or_insert_with(|| WorkspaceModel {
                id: workspace_id,
                name,
                output_id: None,
                layout_space: None,
                focused: false,
                visible: false,
                effective_layout: None,
            });
    }

    pub fn set_current_workspace(&mut self, workspace_id: WorkspaceId) {
        self.current_workspace_id = Some(workspace_id.clone());

        for (candidate_id, workspace) in &mut self.workspaces {
            let is_current = *candidate_id == workspace_id;
            workspace.focused = is_current;
            workspace.visible = is_current;
        }
    }

    pub fn upsert_output(
        &mut self,
        output_id: impl Into<OutputId>,
        name: impl Into<String>,
        logical_width: u32,
        logical_height: u32,
        focused_workspace_id: Option<WorkspaceId>,
    ) {
        let output_id = output_id.into();
        let name = name.into();

        self.outputs
            .entry(output_id.clone())
            .and_modify(|output| {
                output.name = name.clone();
                output.logical_width = logical_width;
                output.logical_height = logical_height;
                output.enabled = true;
                output.focused_workspace_id = focused_workspace_id.clone();
            })
            .or_insert_with(|| OutputModel {
                id: output_id.clone(),
                name,
                logical_x: 0,
                logical_y: 0,
                logical_width,
                logical_height,
                enabled: true,
                focused_workspace_id,
            });
    }

    pub fn remove_output(&mut self, output_id: &OutputId) {
        self.outputs.remove(output_id);

        if self.current_output_id.as_ref() == Some(output_id) {
            self.current_output_id = self.outputs.keys().next().cloned();
        }

        for workspace in self.workspaces.values_mut() {
            if workspace.output_id.as_ref() == Some(output_id) {
                workspace.output_id = None;
                workspace.visible = false;
                workspace.focused = false;
            }
        }

        for window in self.windows.values_mut() {
            if window.output_id.as_ref() == Some(output_id) {
                window.output_id = None;
            }
        }
    }

    pub fn attach_workspace_to_output(&mut self, workspace_id: WorkspaceId, output_id: OutputId) {
        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            workspace.output_id = Some(output_id);
            workspace.focused = true;
            workspace.visible = true;
        }
    }

    pub fn set_current_output(&mut self, output_id: OutputId) {
        self.current_output_id = Some(output_id);
    }

    pub fn insert_window(
        &mut self,
        id: WindowId,
        workspace_id: Option<WorkspaceId>,
        output_id: Option<OutputId>,
    ) {
        self.windows.insert(
            id.clone(),
            WindowModel { id: id.clone(), output_id, workspace_id, ..WindowModel::default() },
        );

        self.sync_tiled_window_order_for_window(&id);
    }

    pub fn window_is_on_current_workspace(&self, id: WindowId) -> bool {
        let Some(window) = self.windows.get(&id) else {
            return false;
        };

        match self.current_workspace_id.as_ref() {
            Some(current_workspace_id) => {
                window.workspace_id.as_ref() == Some(current_workspace_id)
            }
            None => true,
        }
    }

    pub fn window_ids_on_current_workspace<I>(&self, window_ids: I) -> Vec<WindowId>
    where
        I: IntoIterator<Item = WindowId>,
    {
        window_ids
            .into_iter()
            .filter(|window_id| self.window_is_on_current_workspace(window_id.clone()))
            .collect()
    }

    pub fn fullscreen_window_on_current_workspace<I>(&self, window_ids: I) -> Option<WindowId>
    where
        I: IntoIterator<Item = WindowId>,
    {
        self.window_ids_on_current_workspace(window_ids)
            .into_iter()
            .find(|window_id| self.windows.get(window_id).is_some_and(|window| window.fullscreen))
    }

    pub fn preferred_focus_window_on_current_workspace<I>(&self, window_ids: I) -> Option<WindowId>
    where
        I: IntoIterator<Item = WindowId>,
    {
        let visible_window_ids = self
            .ordered_window_ids_on_current_workspace(window_ids)
            .into_iter()
            .filter(|window_id| self.window_is_focusable(window_id))
            .collect::<Vec<_>>();

        if let Some(focused_window_id) = self.focused_window_id.clone() {
            if visible_window_ids.contains(&focused_window_id) {
                return Some(focused_window_id);
            }
        }

        if let Some(remembered_window_id) =
            self.remembered_focus_for_scope(&FocusTree::workspace_scope())
            && visible_window_ids.contains(remembered_window_id)
        {
            return Some(remembered_window_id.clone());
        }

        visible_window_ids.into_iter().last()
    }

    pub fn window_is_focusable(&self, id: &WindowId) -> bool {
        self.windows.get(id).is_some_and(|window| !window.closing)
    }

    pub fn window_is_layout_eligible(&self, id: &WindowId) -> bool {
        self.windows.get(id).is_some_and(|window| {
            window.workspace_id.as_ref() == self.current_workspace_id.as_ref()
                && window.mapped
                && !window.closing
                && !window.floating
                && !window.fullscreen
        })
    }

    pub fn focusable_window_ids(&self) -> Vec<WindowId> {
        self.windows
            .iter()
            .filter_map(|(window_id, window)| (!window.closing).then_some(window_id.clone()))
            .collect()
    }

    pub fn preferred_focusable_window_id(&self) -> Option<WindowId> {
        let ordered_window_ids = self.ordered_focusable_window_ids_on_current_workspace(Vec::new());

        if let Some(focused_window_id) = self.focused_window_id.clone()
            && ordered_window_ids.contains(&focused_window_id)
        {
            return Some(focused_window_id);
        }

        ordered_window_ids.into_iter().next_back()
    }

    pub fn ordered_window_ids_on_current_workspace<I>(&self, hinted_window_ids: I) -> Vec<WindowId>
    where
        I: IntoIterator<Item = WindowId>,
    {
        let Some(current_workspace_id) = self.current_workspace_id.as_ref() else {
            return self.ordered_window_ids_for_workspace_with_hints(None, hinted_window_ids);
        };

        self.ordered_window_ids_for_workspace_with_hints(Some(current_workspace_id), hinted_window_ids)
    }

    pub fn ordered_window_ids_for_workspace(&self, workspace_id: &WorkspaceId) -> Vec<WindowId> {
        self.ordered_window_ids_for_workspace_with_hints(Some(workspace_id), Vec::new())
    }

    fn ordered_window_ids_for_workspace_with_hints<I>(
        &self,
        workspace_id: Option<&WorkspaceId>,
        hinted_window_ids: I,
    ) -> Vec<WindowId>
    where
        I: IntoIterator<Item = WindowId>,
    {
        let mut ordered_window_ids = Vec::new();
        let mut seen_window_ids = std::collections::BTreeSet::new();

        if let Some(workspace_id) = workspace_id
            && let Some(window_order) = self.tiled_window_order_by_workspace.get(workspace_id)
        {
            for window_id in window_order {
                if self.has_window(window_id)
                    && self.windows.get(window_id).is_some_and(|window| {
                        window.workspace_id.as_ref() == Some(workspace_id)
                    })
                    && seen_window_ids.insert(window_id.clone())
                {
                    ordered_window_ids.push(window_id.clone());
                }
            }
        }

        if let Some(focus_tree) = self.focus_tree.as_ref() {
            for window_id in focus_tree.ordered_window_ids() {
                if self.has_window(window_id)
                    && workspace_id.is_none_or(|workspace_id| {
                        self.windows.get(window_id).is_some_and(|window| {
                            window.workspace_id.as_ref() == Some(workspace_id)
                        })
                    })
                    && seen_window_ids.insert(window_id.clone())
                {
                    ordered_window_ids.push(window_id.clone());
                }
            }
        }

        for window_id in hinted_window_ids {
            if self.has_window(&window_id)
                && workspace_id.is_none_or(|workspace_id| {
                    self.windows.get(&window_id).is_some_and(|window| {
                        window.workspace_id.as_ref() == Some(workspace_id)
                    })
                })
                && seen_window_ids.insert(window_id.clone())
            {
                ordered_window_ids.push(window_id);
            }
        }

        for window_id in self.windows.keys() {
            if workspace_id.is_none_or(|workspace_id| {
                self.windows.get(window_id).is_some_and(|window| {
                    window.workspace_id.as_ref() == Some(workspace_id)
                })
            })
                && seen_window_ids.insert(window_id.clone())
            {
                ordered_window_ids.push(window_id.clone());
            }
        }

        ordered_window_ids
    }

    pub fn ordered_focusable_window_ids_on_current_workspace<I>(
        &self,
        hinted_window_ids: I,
    ) -> Vec<WindowId>
    where
        I: IntoIterator<Item = WindowId>,
    {
        self.ordered_window_ids_on_current_workspace(hinted_window_ids)
            .into_iter()
            .filter(|window_id| self.window_is_focus_cycle_candidate(window_id))
            .collect()
    }

    pub fn window_is_focus_cycle_candidate(&self, id: &WindowId) -> bool {
        self.windows.get(id).is_some_and(|window| {
            !window.closing
                && window.mapped
                && !window.floating
                && !window.fullscreen
                && self
                    .current_workspace_id
                    .as_ref()
                    .is_none_or(|workspace_id| window.workspace_id.as_ref() == Some(workspace_id))
        })
    }

    pub fn set_focus_tree(&mut self, root: Option<&ResolvedLayoutNode>) {
        self.focus_tree = root.map(FocusTree::from_resolved_root);
        self.prune_focus_memory();
    }

    pub fn set_focus_tree_value(&mut self, focus_tree: Option<FocusTree>) {
        self.focus_tree = focus_tree;
        self.prune_focus_memory();
    }

    pub fn set_focus_navigation(
        &mut self,
        navigation_by_scope: BTreeMap<FocusScopePath, FocusScopeNavigation>,
    ) {
        if let Some(focus_tree) = self.focus_tree.as_mut() {
            focus_tree.set_navigation(navigation_by_scope);
        }
    }

    pub fn focus_scope_path(&self, window_id: &WindowId) -> Option<&[FocusScopePath]> {
        self.focus_tree.as_ref()?.scope_path(window_id)
    }

    pub fn focus_scope_descendants(&self, scope_key: &FocusScopePath) -> Option<&[WindowId]> {
        self.focus_tree.as_ref()?.descendants(scope_key)
    }

    pub fn remembered_focus_for_scope(&self, scope_key: &FocusScopePath) -> Option<&WindowId> {
        self.last_focused_window_id_by_scope.get(scope_key)
    }

    pub fn set_window_mapped(&mut self, id: WindowId, mapped: bool) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.mapped = mapped;
        }
    }

    pub fn set_window_workspace(&mut self, id: WindowId, workspace_id: Option<WorkspaceId>) {
        let previous_workspace_id = self.windows.get(&id).and_then(|window| window.workspace_id.clone());

        if let Some(window) = self.windows.get_mut(&id) {
            window.workspace_id = workspace_id.clone();
        }

        if let Some(previous_workspace_id) = previous_workspace_id {
            self.prune_tiled_window_order_for_workspace(&previous_workspace_id);
        }

        if let Some(workspace_id) = workspace_id {
            self.sync_tiled_window_order_for_workspace(&workspace_id);
        }
    }

    pub fn set_window_floating(&mut self, id: WindowId, floating: bool) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.floating = floating;
        }
    }

    pub fn set_window_floating_geometry(&mut self, id: WindowId, geometry: WindowGeometry) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.floating_geometry = Some(geometry);
        }
    }

    pub fn set_window_fullscreen(&mut self, id: WindowId, fullscreen: bool) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.fullscreen = fullscreen;
        }
    }

    pub fn set_workspace_effective_layout(
        &mut self,
        workspace_id: WorkspaceId,
        effective_layout: Option<LayoutRef>,
    ) {
        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            workspace.effective_layout = effective_layout;
        }
    }

    pub fn set_workspace_layout_space(
        &mut self,
        workspace_id: WorkspaceId,
        layout_space: Option<LayoutSpaceBox>,
    ) {
        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            workspace.layout_space = layout_space;
        }
    }

    pub fn set_window_focused(&mut self, focused_id: Option<WindowId>) {
        self.focused_window_id = focused_id.clone();

        for (window_id, window) in &mut self.windows {
            window.focused = Some(window_id.clone()) == self.focused_window_id;
        }

        if let Some(focused_window_id) = focused_id.as_ref() {
            self.remember_focus_for_window(focused_window_id);
        }
    }

    pub fn set_window_closing(&mut self, id: WindowId, closing: bool) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.closing = closing;
        }
    }

    pub fn set_window_identity(
        &mut self,
        id: WindowId,
        title: Option<String>,
        app_id: Option<String>,
        class: Option<String>,
        instance: Option<String>,
        role: Option<String>,
        window_type: Option<String>,
        urgent: bool,
    ) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.title = title;
            window.app_id = app_id;
            window.class = class;
            window.instance = instance;
            window.role = role;
            window.window_type = window_type;
            window.urgent = urgent;
        }
    }

    pub fn remove_window(&mut self, id: WindowId) {
        let workspace_id = self.windows.get(&id).and_then(|window| window.workspace_id.clone());
        self.windows.remove(&id);
        if self.focused_window_id == Some(id) {
            self.focused_window_id = None;
        }
        if let Some(workspace_id) = workspace_id {
            self.prune_tiled_window_order_for_workspace(&workspace_id);
        }
        self.prune_focus_memory();
    }

    pub fn move_tiled_window(&mut self, first_window_id: &WindowId, second_window_id: &WindowId) -> bool {
        let Some(first_window) = self.windows.get(first_window_id) else {
            return false;
        };
        let Some(second_window) = self.windows.get(second_window_id) else {
            return false;
        };

        if !self.window_is_layout_eligible(first_window_id) || !self.window_is_layout_eligible(second_window_id) {
            return false;
        }

        let Some(workspace_id) = first_window.workspace_id.clone() else {
            return false;
        };

        if second_window.workspace_id.as_ref() != Some(&workspace_id) {
            return false;
        }

        self.sync_tiled_window_order_for_workspace(&workspace_id);

        let Some(window_order) = self.tiled_window_order_by_workspace.get_mut(&workspace_id) else {
            return false;
        };
        let Some((first_index, second_index)) = crate::navigation::managed_window_swap_positions(
            window_order,
            first_window_id.clone(),
            second_window_id.clone(),
        ) else {
            return false;
        };

        window_order.swap(first_index, second_index);
        true
    }

    fn remember_focus_for_window(&mut self, window_id: &WindowId) {
        if let Some(focus_tree) = self.focus_tree.as_ref()
            && let Some(scope_path) = focus_tree.scope_path(window_id)
        {
            for scope_key in scope_path {
                self.last_focused_window_id_by_scope.insert(scope_key.clone(), window_id.clone());
            }
        }
    }

    fn prune_focus_memory(&mut self) {
        let Some(focus_tree) = self.focus_tree.as_ref() else {
            self.last_focused_window_id_by_scope.clear();
            return;
        };

        let valid_scope_keys =
            focus_tree.scope_keys().cloned().collect::<std::collections::BTreeSet<_>>();
        self.last_focused_window_id_by_scope.retain(|scope_key, window_id| {
            valid_scope_keys.contains(scope_key)
                && self.windows.contains_key(window_id)
                && focus_tree.contains_window(window_id)
        });
    }

    fn sync_tiled_window_order_for_window(&mut self, window_id: &WindowId) {
        let Some(workspace_id) = self.windows.get(window_id).and_then(|window| window.workspace_id.clone()) else {
            return;
        };

        self.sync_tiled_window_order_for_workspace(&workspace_id);
    }

    fn sync_tiled_window_order_for_workspace(&mut self, workspace_id: &WorkspaceId) {
        let mut existing = self
            .tiled_window_order_by_workspace
            .remove(workspace_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|window_id| {
                self.windows.get(window_id).is_some_and(|window| {
                    window.workspace_id.as_ref() == Some(workspace_id)
                        && window.mapped
                        && !window.closing
                        && !window.floating
                        && !window.fullscreen
                })
            })
            .collect::<Vec<_>>();

        for window_id in self.windows.keys() {
            if self.windows.get(window_id).is_some_and(|window| {
                window.workspace_id.as_ref() == Some(workspace_id)
                    && window.mapped
                    && !window.closing
                    && !window.floating
                    && !window.fullscreen
            }) && !existing.contains(window_id)
            {
                existing.push(window_id.clone());
            }
        }

        if existing.is_empty() {
            self.tiled_window_order_by_workspace.remove(workspace_id);
        } else {
            self.tiled_window_order_by_workspace.insert(workspace_id.clone(), existing);
        }
    }

    fn prune_tiled_window_order_for_workspace(&mut self, workspace_id: &WorkspaceId) {
        self.sync_tiled_window_order_for_workspace(workspace_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_id;

    #[test]
    fn setting_current_workspace_updates_focus_and_visibility() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.upsert_workspace(WorkspaceId("2".to_string()), "2".to_string());

        model.set_current_workspace(WorkspaceId("2".to_string()));

        assert_eq!(model.current_workspace_id, Some(WorkspaceId("2".to_string())));
        assert_eq!(
            model.workspaces.get(&WorkspaceId("1".to_string())).map(|workspace| workspace.focused),
            Some(false)
        );
        assert_eq!(
            model.workspaces.get(&WorkspaceId("2".to_string())).map(|workspace| workspace.focused),
            Some(true)
        );
        assert_eq!(
            model.workspaces.get(&WorkspaceId("1".to_string())).map(|workspace| workspace.visible),
            Some(false)
        );
        assert_eq!(
            model.workspaces.get(&WorkspaceId("2".to_string())).map(|workspace| workspace.visible),
            Some(true)
        );
    }

    #[test]
    fn setting_window_workspace_updates_that_window() {
        let mut model = WmModel::default();
        model.insert_window(window_id(9), Some(WorkspaceId("1".to_string())), None);

        model.set_window_workspace(window_id(9), Some(WorkspaceId("2".to_string())));

        assert_eq!(
            model.windows.get(&window_id(9)).and_then(|window| window.workspace_id.clone()),
            Some(WorkspaceId("2".to_string()))
        );
    }

    #[test]
    fn setting_window_floating_updates_that_window() {
        let mut model = WmModel::default();
        model.insert_window(window_id(10), None, None);

        model.set_window_floating(window_id(10), true);

        assert_eq!(model.windows.get(&window_id(10)).map(|window| window.floating), Some(true));
    }

    #[test]
    fn setting_window_floating_geometry_updates_that_window() {
        let mut model = WmModel::default();
        model.insert_window(window_id(12), None, None);

        model.set_window_floating_geometry(
            window_id(12),
            WindowGeometry { x: 40, y: 50, width: 800, height: 600 },
        );

        assert_eq!(
            model.floating_geometry(&window_id(12)),
            Some(WindowGeometry { x: 40, y: 50, width: 800, height: 600 })
        );
    }

    #[test]
    fn floating_geometry_persists_across_floating_toggles() {
        let mut model = WmModel::default();
        model.insert_window(window_id(13), None, None);
        model.set_window_floating(window_id(13), true);
        model.set_window_floating_geometry(
            window_id(13),
            WindowGeometry { x: 10, y: 20, width: 900, height: 700 },
        );

        model.set_window_floating(window_id(13), false);
        model.set_window_floating(window_id(13), true);

        assert_eq!(
            model.floating_geometry(&window_id(13)),
            Some(WindowGeometry { x: 10, y: 20, width: 900, height: 700 })
        );
    }

    #[test]
    fn setting_window_fullscreen_updates_that_window() {
        let mut model = WmModel::default();
        model.insert_window(window_id(11), None, None);

        model.set_window_fullscreen(window_id(11), true);

        assert_eq!(model.windows.get(&window_id(11)).map(|window| window.fullscreen), Some(true));
    }

    #[test]
    fn fullscreen_window_on_current_workspace_returns_matching_visible_window() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.upsert_workspace(WorkspaceId("2".to_string()), "2".to_string());
        model.set_current_workspace(WorkspaceId("1".to_string()));
        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("2".to_string())), None);
        model.set_window_fullscreen(window_id(1), true);
        model.set_window_fullscreen(window_id(2), true);

        assert_eq!(
            model.fullscreen_window_on_current_workspace([window_id(1), window_id(2)]),
            Some(window_id(1))
        );
    }

    #[test]
    fn layout_eligibility_excludes_floating_and_fullscreen_windows() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.set_current_workspace(WorkspaceId("1".to_string()));

        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(3), Some(WorkspaceId("1".to_string())), None);

        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(2), true);
        model.set_window_mapped(window_id(3), true);
        model.set_window_floating(window_id(2), true);
        model.set_window_fullscreen(window_id(3), true);

        assert!(model.window_is_layout_eligible(&window_id(1)));
        assert!(!model.window_is_layout_eligible(&window_id(2)));
        assert!(!model.window_is_layout_eligible(&window_id(3)));
    }

    #[test]
    fn move_tiled_window_swaps_persistent_workspace_order() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.set_current_workspace(WorkspaceId("1".to_string()));

        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("1".to_string())), None);
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(2), true);
        model.sync_tiled_window_order_for_workspace(&WorkspaceId("1".to_string()));

        assert!(model.move_tiled_window(&window_id(1), &window_id(2)));
        assert_eq!(
            model.ordered_window_ids_for_workspace(&WorkspaceId("1".to_string())),
            vec![window_id(2), window_id(1)]
        );
    }

    #[test]
    fn move_tiled_window_rejects_non_layout_eligible_windows() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.set_current_workspace(WorkspaceId("1".to_string()));

        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("1".to_string())), None);
        model.set_window_mapped(window_id(1), true);
        model.set_window_mapped(window_id(2), true);
        model.set_window_floating(window_id(2), true);
        model.sync_tiled_window_order_for_workspace(&WorkspaceId("1".to_string()));
        let original_order = model.ordered_window_ids_for_workspace(&WorkspaceId("1".to_string()));

        assert!(!model.move_tiled_window(&window_id(1), &window_id(2)));
        assert_eq!(
            model.ordered_window_ids_for_workspace(&WorkspaceId("1".to_string())),
            original_order
        );
    }

    #[test]
    fn upserting_output_assigns_current_workspace() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.set_current_workspace(WorkspaceId("1".to_string()));

        model.upsert_output("winit", "winit", 1280, 720, Some(WorkspaceId("1".to_string())));
        model.attach_workspace_to_output(
            WorkspaceId("1".to_string()),
            OutputId("winit".to_string()),
        );
        model.set_current_output(OutputId("winit".to_string()));

        assert_eq!(model.current_output_id, Some(OutputId("winit".to_string())));
        assert_eq!(
            model.outputs.get(&OutputId("winit".to_string())).map(|output| output.logical_width),
            Some(1280)
        );
        assert_eq!(
            model
                .workspaces
                .get(&WorkspaceId("1".to_string()))
                .and_then(|workspace| workspace.output_id.clone()),
            Some(OutputId("winit".to_string()))
        );
    }

    #[test]
    fn inserting_window_uses_current_workspace_and_output() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.set_current_workspace(WorkspaceId("1".to_string()));
        model.upsert_output("winit", "winit", 1280, 720, Some(WorkspaceId("1".to_string())));
        model.attach_workspace_to_output(
            WorkspaceId("1".to_string()),
            OutputId("winit".to_string()),
        );
        model.set_current_output(OutputId("winit".to_string()));

        model.insert_window(
            window_id(7),
            model.current_workspace_id.clone(),
            model.current_output_id.clone(),
        );

        let window = model.windows.get(&window_id(7)).expect("window missing");
        assert_eq!(window.workspace_id, Some(WorkspaceId("1".to_string())));
        assert_eq!(window.output_id, Some(OutputId("winit".to_string())));
        assert!(!window.mapped);
    }

    #[test]
    fn focusing_window_updates_focus_flags() {
        let mut model = WmModel::default();
        model.insert_window(window_id(1), None, None);
        model.insert_window(window_id(2), None, None);

        model.set_window_focused(Some(window_id(2)));

        assert_eq!(model.focused_window_id, Some(window_id(2)));
        assert_eq!(model.windows.get(&window_id(1)).map(|window| window.focused), Some(false));
        assert_eq!(model.windows.get(&window_id(2)).map(|window| window.focused), Some(true));
    }

    #[test]
    fn setting_window_identity_updates_title_and_app_id() {
        let mut model = WmModel::default();
        model.insert_window(window_id(3), None, None);

        model.set_window_identity(
            window_id(3),
            Some("Terminal".to_string()),
            Some("foot".to_string()),
            Some("foot".to_string()),
            Some("foot".to_string()),
            None,
            None,
            false,
        );

        let window = model.windows.get(&window_id(3)).expect("window missing");
        assert_eq!(window.title.as_deref(), Some("Terminal"));
        assert_eq!(window.app_id.as_deref(), Some("foot"));
        assert_eq!(window.class.as_deref(), Some("foot"));
        assert_eq!(window.instance.as_deref(), Some("foot"));
    }

    #[test]
    fn current_workspace_window_membership_is_explicit() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.upsert_workspace(WorkspaceId("2".to_string()), "2".to_string());
        model.set_current_workspace(WorkspaceId("2".to_string()));
        model.insert_window(
            window_id(1),
            Some(WorkspaceId("1".to_string())),
            Some(OutputId("winit".to_string())),
        );
        model.insert_window(
            window_id(2),
            Some(WorkspaceId("2".to_string())),
            Some(OutputId("winit".to_string())),
        );

        assert!(!model.window_is_on_current_workspace(window_id(1)));
        assert!(model.window_is_on_current_workspace(window_id(2)));
        assert!(!model.window_is_on_current_workspace(window_id(99)));
    }

    #[test]
    fn current_workspace_window_order_preserves_stack_order_input() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.upsert_workspace(WorkspaceId("2".to_string()), "2".to_string());
        model.set_current_workspace(WorkspaceId("2".to_string()));
        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("2".to_string())), None);
        model.insert_window(window_id(3), Some(WorkspaceId("2".to_string())), None);

        let visible =
            model.window_ids_on_current_workspace([window_id(3), window_id(1), window_id(2)]);

        assert_eq!(visible, vec![window_id(3), window_id(2)]);
    }

    #[test]
    fn preferred_focus_window_falls_back_to_last_visible_window() {
        let mut model = WmModel::default();
        model.upsert_workspace(WorkspaceId("1".to_string()), "1".to_string());
        model.upsert_workspace(WorkspaceId("2".to_string()), "2".to_string());
        model.set_current_workspace(WorkspaceId("2".to_string()));
        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("2".to_string())), None);
        model.insert_window(window_id(3), Some(WorkspaceId("2".to_string())), None);
        model.set_window_focused(Some(window_id(1)));

        let focused = model.preferred_focus_window_on_current_workspace([
            window_id(1),
            window_id(2),
            window_id(3),
        ]);

        assert_eq!(focused, Some(window_id(3)));
    }
}
