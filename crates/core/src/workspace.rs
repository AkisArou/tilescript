use crate::wm::WmModel;
use crate::{
    LayoutNodeMeta, RemainingTake, ResolvedLayoutNode, SlotTake, SourceLayoutNode, WindowId,
    WorkspaceId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSelection {
    pub workspace_id: WorkspaceId,
    pub focused_window_id: Option<WindowId>,
}

pub fn ensure_workspace(model: &mut WmModel, name: impl Into<String>) -> WorkspaceId {
    let name = name.into();
    let workspace_id = WorkspaceId(name.clone());
    model.upsert_workspace(workspace_id.clone(), name);
    workspace_id
}

pub fn ensure_default_workspace(model: &mut WmModel, name: impl Into<String>) -> WorkspaceId {
    let workspace_id = ensure_workspace(model, name);
    if model.current_workspace_id.is_none() {
        let window_ids: Vec<_> = model.windows.keys().cloned().collect();
        let _ = request_select_workspace(model, workspace_id.clone(), window_ids);
    }
    workspace_id
}

pub fn request_select_workspace<I>(
    model: &mut WmModel,
    workspace_id: WorkspaceId,
    window_ids: I,
) -> Option<WorkspaceSelection>
where
    I: IntoIterator<Item = WindowId>,
{
    if !model.workspaces.contains_key(&workspace_id) {
        return None;
    }

    model.set_current_workspace(workspace_id.clone());
    let focused_window_id = model.preferred_focus_window_on_current_workspace(window_ids);

    Some(WorkspaceSelection { workspace_id, focused_window_id })
}

pub fn request_select_next_workspace<I>(
    model: &mut WmModel,
    window_ids: I,
) -> Option<WorkspaceSelection>
where
    I: IntoIterator<Item = WindowId>,
{
    let next_workspace_id = match model.current_workspace_id.as_ref() {
        Some(current_id) => model
            .workspaces
            .keys()
            .find(|workspace_id| *workspace_id > current_id)
            .cloned()
            .or_else(|| model.workspaces.keys().next().cloned()),
        None => model.workspaces.keys().next().cloned(),
    }?;

    request_select_workspace(model, next_workspace_id, window_ids)
}

pub fn request_select_previous_workspace<I>(
    model: &mut WmModel,
    window_ids: I,
) -> Option<WorkspaceSelection>
where
    I: IntoIterator<Item = WindowId>,
{
    let previous_workspace_id = match model.current_workspace_id.as_ref() {
        Some(current_id) => model
            .workspaces
            .keys()
            .rev()
            .find(|workspace_id| *workspace_id < current_id)
            .cloned()
            .or_else(|| model.workspaces.keys().next_back().cloned()),
        None => model.workspaces.keys().next_back().cloned(),
    }?;

    request_select_workspace(model, previous_workspace_id, window_ids)
}

pub fn place_new_window(model: &mut WmModel, window_id: WindowId) -> WindowId {
    let workspace_id = model.current_workspace_id.clone();
    let output_id = model.current_output_id.clone();
    model.insert_window(window_id.clone(), workspace_id, output_id);
    window_id
}

pub fn fallback_master_stack_layout_tree() -> SourceLayoutNode {
    SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![
            SourceLayoutNode::Group {
                meta: LayoutNodeMeta { id: Some("main".into()), ..LayoutNodeMeta::default() },
                children: vec![SourceLayoutNode::Slot {
                    meta: LayoutNodeMeta::default(),
                    window_match: None,
                    take: SlotTake::Count(1),
                }],
            },
            SourceLayoutNode::Group {
                meta: LayoutNodeMeta { id: Some("stack".into()), ..LayoutNodeMeta::default() },
                children: vec![SourceLayoutNode::Slot {
                    meta: LayoutNodeMeta::default(),
                    window_match: None,
                    take: SlotTake::Remaining(RemainingTake::Remaining),
                }],
            },
        ],
    }
}

pub fn flat_workspace_root<I>(visible_window_ids: I) -> ResolvedLayoutNode
where
    I: IntoIterator<Item = WindowId>,
{
    ResolvedLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: visible_window_ids
            .into_iter()
            .map(|window_id| ResolvedLayoutNode::Window {
                meta: LayoutNodeMeta::default(),
                window_id: Some(window_id),
                children: Vec::new(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OutputId, WorkspaceId, window_id};

    #[test]
    fn ensuring_default_workspace_creates_and_selects_it() {
        let mut model = WmModel::default();

        let workspace_id = ensure_default_workspace(&mut model, "1");

        assert_eq!(workspace_id, WorkspaceId("1".to_string()));
        assert_eq!(model.current_workspace_id, Some(WorkspaceId("1".to_string())));
        assert_eq!(
            model.workspaces.get(&WorkspaceId("1".to_string())).map(|workspace| workspace.focused),
            Some(true)
        );
    }

    #[test]
    fn selecting_workspace_updates_focus_and_visibility() {
        let mut model = WmModel::default();
        ensure_workspace(&mut model, "1");
        ensure_workspace(&mut model, "2");
        ensure_default_workspace(&mut model, "1");

        let selected =
            request_select_workspace(&mut model, WorkspaceId("2".to_string()), Vec::new());

        assert_eq!(
            selected,
            Some(WorkspaceSelection {
                workspace_id: WorkspaceId("2".to_string()),
                focused_window_id: None,
            })
        );
        assert_eq!(model.current_workspace_id, Some(WorkspaceId("2".to_string())));
        assert_eq!(
            model.workspaces.get(&WorkspaceId("1".to_string())).map(|workspace| workspace.focused),
            Some(false)
        );
        assert_eq!(
            model.workspaces.get(&WorkspaceId("2".to_string())).map(|workspace| workspace.visible),
            Some(true)
        );
    }

    #[test]
    fn selecting_next_workspace_advances_and_wraps() {
        let mut model = WmModel::default();
        ensure_workspace(&mut model, "1");
        ensure_workspace(&mut model, "2");
        ensure_workspace(&mut model, "3");
        ensure_default_workspace(&mut model, "2");

        let next = request_select_next_workspace(&mut model, Vec::new());
        assert_eq!(
            next,
            Some(WorkspaceSelection {
                workspace_id: WorkspaceId("3".to_string()),
                focused_window_id: None,
            })
        );

        let wrapped = request_select_next_workspace(&mut model, Vec::new());
        assert_eq!(
            wrapped,
            Some(WorkspaceSelection {
                workspace_id: WorkspaceId("1".to_string()),
                focused_window_id: None,
            })
        );
    }

    #[test]
    fn selecting_previous_workspace_rewinds_and_wraps() {
        let mut model = WmModel::default();
        ensure_workspace(&mut model, "1");
        ensure_workspace(&mut model, "2");
        ensure_workspace(&mut model, "3");
        ensure_default_workspace(&mut model, "2");

        let previous = request_select_previous_workspace(&mut model, Vec::new());
        assert_eq!(
            previous,
            Some(WorkspaceSelection {
                workspace_id: WorkspaceId("1".to_string()),
                focused_window_id: None,
            })
        );

        let wrapped = request_select_previous_workspace(&mut model, Vec::new());
        assert_eq!(
            wrapped,
            Some(WorkspaceSelection {
                workspace_id: WorkspaceId("3".to_string()),
                focused_window_id: None,
            })
        );
    }

    #[test]
    fn request_select_workspace_returns_preferred_focus_for_selected_workspace() {
        let mut model = WmModel::default();
        ensure_workspace(&mut model, "1");
        ensure_workspace(&mut model, "2");
        model.insert_window(window_id(1), Some(WorkspaceId("1".to_string())), None);
        model.insert_window(window_id(2), Some(WorkspaceId("2".to_string())), None);
        model.insert_window(window_id(3), Some(WorkspaceId("2".to_string())), None);
        model.set_window_focused(Some(window_id(1)));

        let selection = request_select_workspace(
            &mut model,
            WorkspaceId("2".to_string()),
            [window_id(1), window_id(2), window_id(3)],
        );

        assert_eq!(
            selection,
            Some(WorkspaceSelection {
                workspace_id: WorkspaceId("2".to_string()),
                focused_window_id: Some(window_id(3)),
            })
        );
    }

    #[test]
    fn places_new_window_on_current_workspace_and_output() {
        let mut model = WmModel::default();
        ensure_default_workspace(&mut model, "1");
        model.current_output_id = Some(OutputId("winit".to_string()));

        let placed_window_id = place_new_window(&mut model, window_id(5));

        assert_eq!(placed_window_id, window_id(5));
        let window = model.windows.get(&window_id(5)).expect("window missing");
        assert_eq!(window.workspace_id, Some(WorkspaceId("1".to_string())));
        assert_eq!(window.output_id, Some(OutputId("winit".to_string())));
    }

    #[test]
    fn places_new_window_even_without_current_workspace_or_output() {
        let mut model = WmModel::default();

        let placed_window_id = place_new_window(&mut model, window_id(6));

        assert_eq!(placed_window_id, window_id(6));
        let window = model.windows.get(&window_id(6)).expect("window missing");
        assert_eq!(window.workspace_id, None);
        assert_eq!(window.output_id, None);
    }

    #[test]
    fn fallback_master_stack_layout_tree_contains_main_and_stack_groups() {
        let SourceLayoutNode::Workspace { children, .. } = fallback_master_stack_layout_tree()
        else {
            panic!("expected workspace root");
        };

        assert_eq!(children.len(), 2);
        assert!(matches!(
            &children[0],
            SourceLayoutNode::Group { meta, .. } if meta.id.as_deref() == Some("main")
        ));
        assert!(matches!(
            &children[1],
            SourceLayoutNode::Group { meta, .. } if meta.id.as_deref() == Some("stack")
        ));
    }

    #[test]
    fn flat_workspace_root_materializes_windows_in_order() {
        let ResolvedLayoutNode::Workspace { children, .. } =
            flat_workspace_root([window_id(1), window_id(2)])
        else {
            panic!("expected workspace root");
        };

        assert_eq!(children.len(), 2);
        assert!(matches!(
            &children[0],
            ResolvedLayoutNode::Window { window_id: Some(id), .. } if id == &window_id(1)
        ));
        assert!(matches!(
            &children[1],
            ResolvedLayoutNode::Window { window_id: Some(id), .. } if id == &window_id(2)
        ));
    }
}
