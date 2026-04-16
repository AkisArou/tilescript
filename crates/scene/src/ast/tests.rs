use tilescript_core::WindowId;
use tilescript_core::snapshot::WindowSnapshot;
use tilescript_core::types::{WindowMode, WindowShell};
use tilescript_core::{LayoutNodeMeta, LayoutNodeType, MatchClause, MatchKey, RemainingTake};

use super::*;
use crate::matching::MatchParseError;
use tilescript_core::{ResolvedLayoutNode, SlotTake, SourceLayoutNode, WindowMatch};

fn window(id: &str, app_id: &str, title: &str) -> WindowSnapshot {
    WindowSnapshot {
        id: WindowId::from(id),
        shell: WindowShell::Wayland,
        app_id: Some(app_id.into()),
        title: Some(title.into()),
        class: None,
        instance: None,
        role: None,
        window_type: None,
        mapped: true,
        mode: WindowMode::Tiled,
        focused: false,
        urgent: false,
        closing: false,
        output_id: None,
        workspace_id: None,
        workspaces: vec![],
    }
}

#[test]
fn rejects_non_workspace_root() {
    let tree = SourceLayoutNode::Group { meta: LayoutNodeMeta::default(), children: vec![] };

    let error = ValidatedLayoutTree::new(tree).unwrap_err();

    assert_eq!(error, LayoutValidationError::RootMustBeWorkspace);
}

#[test]
fn rejects_duplicate_ids() {
    let tree = SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta { id: Some("root".into()), ..LayoutNodeMeta::default() },
        children: vec![
            SourceLayoutNode::Group {
                meta: LayoutNodeMeta { id: Some("dup".into()), ..LayoutNodeMeta::default() },
                children: vec![],
            },
            SourceLayoutNode::Window {
                meta: LayoutNodeMeta { id: Some("dup".into()), ..LayoutNodeMeta::default() },
                window_match: None,
            },
        ],
    };

    let error = ValidatedLayoutTree::new(tree).unwrap_err();

    assert_eq!(error, LayoutValidationError::DuplicateId { id: "dup".into() });
}

#[test]
fn rejects_nested_workspace_nodes() {
    let tree = SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![],
        }],
    };

    let error = ValidatedLayoutTree::new(tree).unwrap_err();

    assert_eq!(
        error,
        LayoutValidationError::InvalidChild {
            parent: LayoutNodeType::Workspace,
            child: LayoutNodeType::Workspace,
        }
    );
}

#[test]
fn rejects_zero_slot_take() {
    let tree = SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Slot {
            meta: LayoutNodeMeta::default(),
            window_match: None,
            take: SlotTake::Count(0),
        }],
    };

    let error = ValidatedLayoutTree::new(tree).unwrap_err();

    assert_eq!(error, LayoutValidationError::InvalidSlotTake);
}

#[test]
fn accepts_non_empty_match_clauses() {
    let tree = SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Window {
            meta: LayoutNodeMeta::default(),
            window_match: Some(WindowMatch {
                clauses: vec![MatchClause { key: MatchKey::AppId, value: "firefox".into() }],
            }),
        }],
    };

    let validated = ValidatedLayoutTree::new(tree);

    assert!(validated.is_ok());
}

#[test]
fn normalizes_authored_match_expression_before_validation() {
    let tree = ValidatedLayoutTree::from_authored(AuthoredLayoutNode::Workspace {
        meta: AuthoredNodeMeta::default(),
        children: vec![AuthoredLayoutNode::Window {
            meta: AuthoredNodeMeta::default(),
            match_expr: Some("app_id=\"firefox\" title=\"Mozilla Firefox\"".into()),
        }],
    })
    .unwrap();

    assert_eq!(
        tree.root,
        SourceLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![SourceLayoutNode::Window {
                meta: LayoutNodeMeta::default(),
                window_match: Some(WindowMatch {
                    clauses: vec![
                        MatchClause { key: MatchKey::AppId, value: "firefox".into() },
                        MatchClause { key: MatchKey::Title, value: "Mozilla Firefox".into() },
                    ],
                }),
            }],
        }
    );
}

#[test]
fn authored_invalid_match_bubbles_up_as_validation_error() {
    let error = ValidatedLayoutTree::from_authored(AuthoredLayoutNode::Workspace {
        meta: AuthoredNodeMeta::default(),
        children: vec![AuthoredLayoutNode::Window {
            meta: AuthoredNodeMeta::default(),
            match_expr: Some("app_id=firefox".into()),
        }],
    })
    .unwrap_err();

    assert_eq!(
        error,
        LayoutValidationError::InvalidMatch {
            source: MatchParseError::ExpectedQuotedValue { key: "app_id".into() },
        }
    );
}

#[test]
fn resolve_keeps_unmatched_window_node_as_empty_runtime_leaf() {
    let tree = ValidatedLayoutTree::new(SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Window {
            meta: LayoutNodeMeta { id: Some("main".into()), ..LayoutNodeMeta::default() },
            window_match: Some(WindowMatch {
                clauses: vec![MatchClause { key: MatchKey::AppId, value: "firefox".into() }],
            }),
        }],
    })
    .unwrap();

    let resolved = tree.resolve(&[window("w1", "alacritty", "Terminal")]).unwrap();

    assert_eq!(
        resolved.root,
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![ResolvedLayoutNode::Window {
                meta: LayoutNodeMeta { id: Some("main".into()), ..LayoutNodeMeta::default() },
                window_id: None,
                children: vec![],
            }],
        }
    );
}

#[test]
fn resolve_slot_expands_into_multiple_runtime_windows_in_order() {
    let tree = ValidatedLayoutTree::new(SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Slot {
            meta: LayoutNodeMeta { class: vec!["stack".into()], ..LayoutNodeMeta::default() },
            window_match: Some(WindowMatch {
                clauses: vec![MatchClause { key: MatchKey::AppId, value: "firefox".into() }],
            }),
            take: SlotTake::Count(2),
        }],
    })
    .unwrap();

    let resolved = tree
        .resolve(&[
            window("w1", "firefox", "one"),
            window("w2", "firefox", "two"),
            window("w3", "firefox", "three"),
        ])
        .unwrap();

    assert_eq!(
        resolved.root,
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta {
                        class: vec!["stack".into()],
                        data: [
                            ("app_id".into(), "firefox".into()),
                            ("shell".into(), "wayland".into()),
                            ("title".into(), "one".into()),
                        ]
                        .into_iter()
                        .collect(),
                        ..LayoutNodeMeta::default()
                    },
                    window_id: Some(WindowId::from("w1")),
                    children: vec![],
                },
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta {
                        class: vec!["stack".into()],
                        data: [
                            ("app_id".into(), "firefox".into()),
                            ("shell".into(), "wayland".into()),
                            ("title".into(), "two".into()),
                        ]
                        .into_iter()
                        .collect(),
                        ..LayoutNodeMeta::default()
                    },
                    window_id: Some(WindowId::from("w2")),
                    children: vec![],
                },
            ],
        }
    );
}

#[test]
fn resolve_later_nodes_only_see_unclaimed_windows() {
    let tree = ValidatedLayoutTree::new(SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![
            SourceLayoutNode::Window {
                meta: LayoutNodeMeta { id: Some("primary".into()), ..LayoutNodeMeta::default() },
                window_match: Some(WindowMatch {
                    clauses: vec![MatchClause { key: MatchKey::AppId, value: "firefox".into() }],
                }),
            },
            SourceLayoutNode::Slot {
                meta: LayoutNodeMeta { id: Some("rest".into()), ..LayoutNodeMeta::default() },
                window_match: Some(WindowMatch {
                    clauses: vec![MatchClause { key: MatchKey::AppId, value: "firefox".into() }],
                }),
                take: SlotTake::Remaining(RemainingTake::Remaining),
            },
        ],
    })
    .unwrap();

    let resolved = tree
        .resolve(&[
            window("w1", "firefox", "one"),
            window("w2", "firefox", "two"),
            window("w3", "alacritty", "three"),
        ])
        .unwrap();

    assert_eq!(
        resolved.root,
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta {
                        id: Some("primary".into()),
                        data: [
                            ("app_id".into(), "firefox".into()),
                            ("shell".into(), "wayland".into()),
                            ("title".into(), "one".into()),
                        ]
                        .into_iter()
                        .collect(),
                        ..LayoutNodeMeta::default()
                    },
                    window_id: Some(WindowId::from("w1")),
                    children: vec![],
                },
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta {
                        id: Some("rest".into()),
                        data: [
                            ("app_id".into(), "firefox".into()),
                            ("shell".into(), "wayland".into()),
                            ("title".into(), "two".into()),
                        ]
                        .into_iter()
                        .collect(),
                        ..LayoutNodeMeta::default()
                    },
                    window_id: Some(WindowId::from("w2")),
                    children: vec![],
                },
            ],
        }
    );
}

#[test]
fn resolve_focus_repro_slots_keep_second_window_in_main_column() {
    let tree = ValidatedLayoutTree::new(SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![
            SourceLayoutNode::Group {
                meta: LayoutNodeMeta {
                    id: Some("main-column".into()),
                    ..LayoutNodeMeta::default()
                },
                children: vec![
                    SourceLayoutNode::Slot {
                        meta: LayoutNodeMeta {
                            id: Some("main-top".into()),
                            ..LayoutNodeMeta::default()
                        },
                        window_match: None,
                        take: SlotTake::Count(1),
                    },
                    SourceLayoutNode::Slot {
                        meta: LayoutNodeMeta {
                            id: Some("main-bottom".into()),
                            ..LayoutNodeMeta::default()
                        },
                        window_match: None,
                        take: SlotTake::Count(1),
                    },
                ],
            },
            SourceLayoutNode::Group {
                meta: LayoutNodeMeta {
                    id: Some("side-column".into()),
                    ..LayoutNodeMeta::default()
                },
                children: vec![SourceLayoutNode::Slot {
                    meta: LayoutNodeMeta { id: Some("side".into()), ..LayoutNodeMeta::default() },
                    window_match: None,
                    take: SlotTake::Remaining(RemainingTake::Remaining),
                }],
            },
        ],
    })
    .unwrap();

    let resolved = tree
        .resolve(&[
            window("w1", "foot", "Terminal 2"),
            window("w2", "zen", "Reference"),
            window("w3", "foot", "Terminal 4"),
            window("w4", "foot", "Terminal 5"),
        ])
        .unwrap();

    assert_eq!(
        resolved.root,
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![
                ResolvedLayoutNode::Group {
                    meta: LayoutNodeMeta {
                        id: Some("main-column".into()),
                        ..LayoutNodeMeta::default()
                    },
                    children: vec![
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta {
                                id: Some("main-top".into()),
                                data: [
                                    ("app_id".into(), "foot".into()),
                                    ("shell".into(), "wayland".into()),
                                    ("title".into(), "Terminal 2".into()),
                                ]
                                .into_iter()
                                .collect(),
                                ..LayoutNodeMeta::default()
                            },
                            window_id: Some(WindowId::from("w1")),
                            children: vec![],
                        },
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta {
                                id: Some("main-bottom".into()),
                                data: [
                                    ("app_id".into(), "zen".into()),
                                    ("shell".into(), "wayland".into()),
                                    ("title".into(), "Reference".into()),
                                ]
                                .into_iter()
                                .collect(),
                                ..LayoutNodeMeta::default()
                            },
                            window_id: Some(WindowId::from("w2")),
                            children: vec![],
                        },
                    ],
                },
                ResolvedLayoutNode::Group {
                    meta: LayoutNodeMeta {
                        id: Some("side-column".into()),
                        ..LayoutNodeMeta::default()
                    },
                    children: vec![
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta {
                                id: Some("side".into()),
                                data: [
                                    ("app_id".into(), "foot".into()),
                                    ("shell".into(), "wayland".into()),
                                    ("title".into(), "Terminal 4".into()),
                                ]
                                .into_iter()
                                .collect(),
                                ..LayoutNodeMeta::default()
                            },
                            window_id: Some(WindowId::from("w3")),
                            children: vec![],
                        },
                        ResolvedLayoutNode::Window {
                            meta: LayoutNodeMeta {
                                id: Some("side".into()),
                                data: [
                                    ("app_id".into(), "foot".into()),
                                    ("shell".into(), "wayland".into()),
                                    ("title".into(), "Terminal 5".into()),
                                ]
                                .into_iter()
                                .collect(),
                                ..LayoutNodeMeta::default()
                            },
                            window_id: Some(WindowId::from("w4")),
                            children: vec![],
                        },
                    ],
                },
            ],
        }
    );
}

#[test]
fn resolve_window_meta_adds_runtime_state_classes() {
    let tree = ValidatedLayoutTree::new(SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Window {
            meta: LayoutNodeMeta::default(),
            window_match: None,
        }],
    })
    .unwrap();

    let mut floating = window("w1", "foot", "Terminal");
    floating.focused = true;
    floating.urgent = true;
    floating.mode = WindowMode::Floating { rect: None };

    let resolved = tree.resolve(&[floating]).unwrap();

    let ResolvedLayoutNode::Workspace { children, .. } = resolved.root else {
        panic!("expected workspace root");
    };
    let ResolvedLayoutNode::Window { meta, .. } = &children[0] else {
        panic!("expected window node");
    };

    assert!(meta.class.iter().any(|class| class == "focused"));
    assert!(meta.class.iter().any(|class| class == "urgent"));
    assert!(meta.class.iter().any(|class| class == "floating"));
    assert!(!meta.class.iter().any(|class| class == "fullscreen"));
}

#[test]
fn resolve_window_meta_adds_fullscreen_class() {
    let tree = ValidatedLayoutTree::new(SourceLayoutNode::Workspace {
        meta: LayoutNodeMeta::default(),
        children: vec![SourceLayoutNode::Window {
            meta: LayoutNodeMeta::default(),
            window_match: None,
        }],
    })
    .unwrap();

    let mut fullscreen = window("w1", "foot", "Terminal");
    fullscreen.mode = WindowMode::Fullscreen;

    let resolved = tree.resolve(&[fullscreen]).unwrap();

    let ResolvedLayoutNode::Workspace { children, .. } = resolved.root else {
        panic!("expected workspace root");
    };
    let ResolvedLayoutNode::Window { meta, .. } = &children[0] else {
        panic!("expected window node");
    };

    assert!(meta.class.iter().any(|class| class == "fullscreen"));
    assert!(!meta.class.iter().any(|class| class == "floating"));
}
