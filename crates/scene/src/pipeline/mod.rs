use crate::CompiledKeyframesRule;
use crate::css::{CssParseError, CssValueError, StyledLayoutTree, parse_stylesheet};
pub use crate::layout_calc::{LaidOutNode, LaidOutTree};
use crate::scene::{SceneRequest, SceneResponse};
use crate::style_tree::build_styled_layout_tree_from_sheet_with_resize_state;
use serde::Serialize;
use hypreact_core::ResolvedLayoutNode;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug, Clone)]
struct CachedStylesheet {
    source: String,
    sheet: crate::css::CompiledStyleSheet,
}

#[derive(Debug, Default, Clone)]
pub struct SceneCache {
    stylesheets: HashMap<String, CachedStylesheet>,
    scenes: HashMap<String, SceneResponse>,
}

impl SceneCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        debug!(
            cached_stylesheets = self.stylesheets.len(),
            cached_scenes = self.scenes.len(),
            "clearing scene cache"
        );
        self.stylesheets.clear();
        self.scenes.clear();
    }

    pub fn precompile_layout(
        &mut self,
        layout_name: impl Into<String>,
        stylesheet_source: &str,
    ) -> Result<(), LayoutPipelineError> {
        let layout_name = layout_name.into();

        match self.stylesheets.get(&layout_name) {
            Some(cached) if cached.source == stylesheet_source => {
                debug!(layout = %layout_name, "scene cache hit; stylesheet unchanged");
                Ok(())
            }
            _ => {
                debug!(layout = %layout_name, bytes = stylesheet_source.len(), "compiling stylesheet for scene cache");
                let sheet = compile_stylesheet(stylesheet_source)?;
                self.stylesheets.insert(
                    layout_name,
                    CachedStylesheet { source: stylesheet_source.to_owned(), sheet },
                );
                Ok(())
            }
        }
    }

    pub fn compute_layout_from_request(
        &mut self,
        request: &SceneRequest,
    ) -> Result<SceneResponse, LayoutPipelineError> {
        let scene_key = scene_request_key(request);
        if let Some(scene_key) = scene_key.as_ref()
            && let Some(cached) = self.scenes.get(scene_key)
        {
            debug!(
                layout = request.layout_name.as_deref().unwrap_or("__default__"),
                "scene cache hit; request unchanged"
            );
            return Ok(cached.clone());
        }

        let layout_name = request.layout_name.as_deref().unwrap_or("__default__");
        debug!(
            layout = layout_name,
            width = request.space.width,
            height = request.space.height,
            "computing layout from scene request"
        );
        let stylesheet_source = request.stylesheets.combined_source();
        self.precompile_layout(layout_name, &stylesheet_source)?;

        let sheet = self
            .stylesheets
            .get(layout_name)
            .map(|cached| &cached.sheet)
            .expect("scene cache entry must exist after successful precompile");

        let response = compute_layout_from_request_with_sheet(request, sheet)?;
        if let Some(scene_key) = scene_key {
            self.scenes.insert(scene_key, response.clone());
        }
        Ok(response)
    }

    pub fn keyframes_for_layout(&self, layout_name: &str) -> Vec<CompiledKeyframesRule> {
        self.stylesheets
            .get(layout_name)
            .map(|cached| cached.sheet.keyframes.clone())
            .unwrap_or_default()
    }
}

#[derive(Serialize)]
struct SceneRequestKey<'a> {
    layout_name: Option<&'a str>,
    root: &'a ResolvedLayoutNode,
    stylesheets: &'a hypreact_core::runtime::prepared_layout::PreparedStylesheets,
    width: f32,
    height: f32,
    resize_state: &'a hypreact_core::resize::WorkspaceResizeState,
}

fn scene_request_key(request: &SceneRequest) -> Option<String> {
    serde_json::to_string(&SceneRequestKey {
        layout_name: request.layout_name.as_deref(),
        root: &request.root,
        stylesheets: &request.stylesheets,
        width: request.space.width,
        height: request.space.height,
        resize_state: &request.resize_state,
    })
    .ok()
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum LayoutPipelineError {
    #[error(transparent)]
    CssParse(#[from] CssParseError),
    #[error(transparent)]
    CssValue(#[from] CssValueError),
    #[error(transparent)]
    LayoutCalc(#[from] crate::layout_calc::LayoutCalcError),
}

pub fn compile_stylesheet(
    stylesheet_source: &str,
) -> Result<crate::css::CompiledStyleSheet, LayoutPipelineError> {
    parse_stylesheet(stylesheet_source).map_err(LayoutPipelineError::from)
}

pub fn build_styled_layout_tree(
    root: &ResolvedLayoutNode,
    stylesheet_source: &str,
) -> Result<StyledLayoutTree, LayoutPipelineError> {
    let sheet = parse_stylesheet(stylesheet_source)?;
    crate::style_tree::build_styled_layout_tree_from_sheet(root, &sheet)
        .map_err(LayoutPipelineError::from)
}

pub fn compute_layout(
    root: &ResolvedLayoutNode,
    stylesheet_source: &str,
    width: f32,
    height: f32,
) -> Result<LaidOutTree, LayoutPipelineError> {
    let sheet = compile_stylesheet(stylesheet_source)?;
    compute_layout_from_sheet(root, &sheet, width, height)
}

pub fn compute_layout_from_sheet(
    root: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
    width: f32,
    height: f32,
) -> Result<LaidOutTree, LayoutPipelineError> {
    let styled = build_styled_layout_tree_from_sheet_with_resize_state(
        root,
        sheet,
        &hypreact_core::resize::WorkspaceResizeState::default(),
    )
    .map_err(LayoutPipelineError::from)?;
    compute_layout_from_styled(&styled, width, height)
}

pub fn compute_layout_from_styled(
    styled: &StyledLayoutTree,
    width: f32,
    height: f32,
) -> Result<LaidOutTree, LayoutPipelineError> {
    crate::layout_calc::compute_layout_from_styled(styled, width, height)
        .map_err(LayoutPipelineError::from)
}

pub fn compute_layout_from_request(
    request: &SceneRequest,
) -> Result<SceneResponse, LayoutPipelineError> {
    let sheet = compile_stylesheet(&request.stylesheets.combined_source())?;
    let laid_out = compute_layout_from_sheet(
        &request.root,
        &sheet,
        request.space.width,
        request.space.height,
    )?;

    Ok(SceneResponse { root: laid_out.snapshot() })
}

pub fn compute_layout_from_request_with_sheet(
    request: &SceneRequest,
    sheet: &crate::css::CompiledStyleSheet,
) -> Result<SceneResponse, LayoutPipelineError> {
    let styled = build_styled_layout_tree_from_sheet_with_resize_state(
        &request.root,
        sheet,
        &request.resize_state,
    )
    .map_err(LayoutPipelineError::from)?;
    let laid_out = compute_layout_from_styled(&styled, request.space.width, request.space.height)?;

    Ok(SceneResponse { root: laid_out.snapshot() })
}

pub fn build_styled_layout_tree_from_sheet(
    root: &ResolvedLayoutNode,
    sheet: &crate::css::CompiledStyleSheet,
) -> Result<StyledLayoutTree, CssValueError> {
    crate::style_tree::build_styled_layout_tree_from_sheet(root, sheet)
}

#[cfg(test)]
mod tests {
    use hypreact_core::{OutputId, WindowId, WorkspaceId};

    use super::*;
    use crate::css::{Display, FlexDirectionValue, LengthPercentage, SizeValue};
    use crate::scene::{LayoutSnapshotNode, SceneNodeStyle, SceneResponse};
    use hypreact_core::{LayoutNodeMeta, LayoutRect, LayoutSpace, ResolvedLayoutNode};

    fn sample_tree() -> ResolvedLayoutNode {
        ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta { class: vec!["root".into()], ..LayoutNodeMeta::default() },
            children: vec![ResolvedLayoutNode::Window {
                meta: LayoutNodeMeta {
                    id: Some("main".into()),
                    class: vec!["stack".into()],
                    ..LayoutNodeMeta::default()
                },
                window_id: Some(WindowId::from("win-1")),
                children: vec![],
            }],
        }
    }

    #[test]
    fn pipeline_builds_computed_styles_for_each_node() {
        let tree = sample_tree();
        let styled = build_styled_layout_tree(
            &tree,
            "workspace { display: flex; flex-direction: row; } #main { width: 60%; }",
        )
        .unwrap();

        assert_eq!(styled.root.computed.display, Some(Display::Flex));
        assert_eq!(styled.root.computed.flex_direction, Some(FlexDirectionValue::Row));
        assert_eq!(styled.root.children.len(), 1);
        assert_eq!(
            styled.root.children[0].computed.width,
            Some(SizeValue::LengthPercentage(LengthPercentage::Percent(60.0)))
        );
    }

    #[test]
    fn pipeline_surfaces_stylesheet_parse_errors() {
        let tree = sample_tree();
        let error = build_styled_layout_tree(&tree, "slot { display: flex; }").unwrap_err();

        assert_eq!(
            error,
            LayoutPipelineError::CssParse(CssParseError::UnsupportedSelector {
                selector: "slot".into(),
            })
        );
    }

    #[test]
    fn pipeline_computes_basic_layout_geometry() {
        let tree = ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta { id: Some("left".into()), ..LayoutNodeMeta::default() },
                    window_id: Some(WindowId::from("w1")),
                    children: vec![],
                },
                ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta { id: Some("right".into()), ..LayoutNodeMeta::default() },
                    window_id: Some(WindowId::from("w2")),
                    children: vec![],
                },
            ],
        };

        let laid_out = compute_layout(
            &tree,
            "workspace { display: flex; flex-direction: row; width: 800px; height: 600px; } #left { width: 200px; } #right { flex-grow: 1; }",
            800.0,
            600.0,
        )
        .unwrap();

        assert_eq!(laid_out.root.geometry.width, 800.0);
        assert_eq!(laid_out.root.geometry.height, 600.0);
        assert_eq!(laid_out.root.children.len(), 2);
        assert_eq!(laid_out.root.children[0].geometry.x, 0.0);
        assert_eq!(laid_out.root.children[0].geometry.width, 200.0);
        assert_eq!(laid_out.root.children[1].geometry.x, 200.0);
        assert_eq!(laid_out.root.children[1].geometry.width, 600.0);
    }

    #[test]
    fn pipeline_handles_gap_padding_and_nested_groups() {
        let tree = ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![ResolvedLayoutNode::Group {
                meta: LayoutNodeMeta { id: Some("stack".into()), ..LayoutNodeMeta::default() },
                children: vec![
                    ResolvedLayoutNode::Window {
                        meta: LayoutNodeMeta { id: Some("a".into()), ..LayoutNodeMeta::default() },
                        window_id: Some(WindowId::from("w1")),
                        children: vec![],
                    },
                    ResolvedLayoutNode::Window {
                        meta: LayoutNodeMeta { id: Some("b".into()), ..LayoutNodeMeta::default() },
                        window_id: Some(WindowId::from("w2")),
                        children: vec![],
                    },
                ],
            }],
        };

        let laid_out = compute_layout(
            &tree,
            "workspace { display: flex; width: 500px; height: 300px; padding: 10px; } #stack { display: flex; flex-direction: column; gap: 20px; width: 100%; height: 100%; } #a { height: 80px; } #b { flex-grow: 1; }",
            500.0,
            300.0,
        )
        .unwrap();

        assert_eq!(laid_out.root.children[0].geometry.x, 10.0);
        assert_eq!(laid_out.root.children[0].geometry.y, 10.0);
        assert_eq!(laid_out.root.children[0].geometry.width, 480.0);
        assert_eq!(laid_out.root.children[0].geometry.height, 280.0);
        assert_eq!(laid_out.root.children[0].children[0].geometry.height, 80.0);
        assert_eq!(laid_out.root.children[0].children[1].geometry.y, 110.0);
        assert_eq!(laid_out.root.children[0].children[1].geometry.height, 180.0);
    }

    #[test]
    fn laid_out_tree_converts_to_shared_snapshot_model() {
        let tree = sample_tree();
        let laid_out = compute_layout(
            &tree,
            "workspace { display: flex; width: 400px; height: 300px; } #main { width: 200px; }",
            400.0,
            300.0,
        )
        .unwrap();

        let snapshot = laid_out.snapshot();

        assert_eq!(
            snapshot,
            LayoutSnapshotNode::Workspace {
                meta: LayoutNodeMeta { class: vec!["root".into()], ..LayoutNodeMeta::default() },
                rect: LayoutRect { x: 0.0, y: 0.0, width: 400.0, height: 300.0 },
                styles: Some(SceneNodeStyle {
                    layout: crate::css::ComputedStyle {
                        display: Some(Display::Flex),
                        width: Some(SizeValue::LengthPercentage(LengthPercentage::Px(400.0))),
                        height: Some(SizeValue::LengthPercentage(LengthPercentage::Px(300.0))),
                        ..crate::css::ComputedStyle::default()
                    },
                }),
                children: vec![LayoutSnapshotNode::Window {
                    meta: LayoutNodeMeta {
                        id: Some("main".into()),
                        class: vec!["stack".into()],
                        ..LayoutNodeMeta::default()
                    },
                    rect: LayoutRect { x: 0.0, y: 0.0, width: 200.0, height: 300.0 },
                    styles: Some(SceneNodeStyle {
                        layout: crate::css::ComputedStyle {
                            width: Some(SizeValue::LengthPercentage(LengthPercentage::Px(200.0))),
                            ..crate::css::ComputedStyle::default()
                        },
                    }),
                    window_id: Some(WindowId::from("win-1")),
                    children: vec![],
                }],
            }
        );
    }

    #[test]
    fn pipeline_supports_shared_layout_request_response_types() {
        let request = SceneRequest {
            workspace_id: WorkspaceId::from("ws-1"),
            output_id: Some(OutputId::from("out-1")),
            layout_name: Some("master-stack".into()),
            root: sample_tree(),
            stylesheets: hypreact_core::runtime::prepared_layout::PreparedStylesheets {
                global: None,
                layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                    path: "layouts/master-stack/index.css".into(),
                    source:
                        "workspace { display: flex; width: 320px; height: 200px; } #main { width: 100px; }"
                            .into(),
                }),
            },
            space: LayoutSpace {
                width: 320.0,
                height: 200.0,
            },
            resize_state: hypreact_core::resize::WorkspaceResizeState::default(),
        };

        let response = compute_layout_from_request(&request).unwrap();

        assert_eq!(
            response,
            SceneResponse {
                root: LayoutSnapshotNode::Workspace {
                    meta: LayoutNodeMeta {
                        class: vec!["root".into()],
                        ..LayoutNodeMeta::default()
                    },
                    rect: LayoutRect { x: 0.0, y: 0.0, width: 320.0, height: 200.0 },
                    styles: Some(SceneNodeStyle {
                        layout: crate::css::ComputedStyle {
                            display: Some(Display::Flex),
                            width: Some(SizeValue::LengthPercentage(LengthPercentage::Px(320.0))),
                            height: Some(SizeValue::LengthPercentage(LengthPercentage::Px(200.0))),
                            ..crate::css::ComputedStyle::default()
                        },
                    }),
                    children: vec![LayoutSnapshotNode::Window {
                        meta: LayoutNodeMeta {
                            id: Some("main".into()),
                            class: vec!["stack".into()],
                            ..LayoutNodeMeta::default()
                        },
                        rect: LayoutRect { x: 0.0, y: 0.0, width: 100.0, height: 200.0 },
                        styles: Some(SceneNodeStyle {
                            layout: crate::css::ComputedStyle {
                                width: Some(SizeValue::LengthPercentage(LengthPercentage::Px(
                                    100.0
                                ))),
                                ..crate::css::ComputedStyle::default()
                            },
                        }),
                        window_id: Some(WindowId::from("win-1")),
                        children: vec![],
                    }],
                },
            }
        );
    }

    #[test]
    fn pipeline_sizes_workspace_root_to_request_space() {
        let tree = ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![ResolvedLayoutNode::Group {
                meta: LayoutNodeMeta { id: Some("frame".into()), ..LayoutNodeMeta::default() },
                children: vec![ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta {
                        id: Some("master".into()),
                        class: vec!["master-slot".into()],
                        ..LayoutNodeMeta::default()
                    },
                    window_id: Some(WindowId::from("win-1")),
                    children: vec![],
                }],
            }],
        };

        let laid_out = compute_layout(
            &tree,
            "#frame { display: flex; padding: 4px; width: 100%; height: 100%; } .master-slot { flex-basis: 0px; flex-grow: 1; min-width: 0px; }",
            1280.0,
            720.0,
        )
        .unwrap();

        assert_eq!(laid_out.root.geometry.width, 1280.0);
        assert_eq!(laid_out.root.geometry.height, 720.0);
        assert_eq!(laid_out.root.children[0].geometry.width, 1280.0);
        assert_eq!(laid_out.root.children[0].geometry.height, 720.0);
        assert_eq!(laid_out.root.children[0].children[0].geometry.width, 1272.0);
        assert_eq!(laid_out.root.children[0].children[0].geometry.height, 712.0);
    }

    #[test]
    fn pipeline_reuses_precompiled_stylesheet_for_multiple_requests() {
        let stylesheet =
            "workspace { display: flex; width: 320px; height: 200px; } #main { width: 100px; }";
        let sheet = compile_stylesheet(stylesheet).unwrap();

        let request = SceneRequest {
            workspace_id: WorkspaceId::from("ws-1"),
            output_id: Some(OutputId::from("out-1")),
            layout_name: Some("master-stack".into()),
            root: sample_tree(),
            stylesheets: hypreact_core::runtime::prepared_layout::PreparedStylesheets {
                global: None,
                layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                    path: "layouts/master-stack/index.css".into(),
                    source: stylesheet.into(),
                }),
            },
            space: LayoutSpace { width: 320.0, height: 200.0 },
            resize_state: hypreact_core::resize::WorkspaceResizeState::default(),
        };

        let response_a = compute_layout_from_request(&request).unwrap();
        let response_b = compute_layout_from_request_with_sheet(&request, &sheet).unwrap();

        assert_eq!(response_a, response_b);
    }

    #[test]
    fn scene_cache_recompiles_when_layout_source_changes() {
        let mut cache = SceneCache::new();

        let request_a = SceneRequest {
            workspace_id: WorkspaceId::from("ws-1"),
            output_id: Some(OutputId::from("out-1")),
            layout_name: Some("master-stack".into()),
            root: sample_tree(),
            stylesheets: hypreact_core::runtime::prepared_layout::PreparedStylesheets {
                global: None,
                layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                    path: "layouts/master-stack/index.css".into(),
                    source:
                        "workspace { display: flex; width: 320px; height: 200px; } #main { width: 100px; }"
                            .into(),
                }),
            },
            space: LayoutSpace {
                width: 320.0,
                height: 200.0,
            },
            resize_state: hypreact_core::resize::WorkspaceResizeState::default(),
        };

        let request_b = SceneRequest {
            stylesheets: hypreact_core::runtime::prepared_layout::PreparedStylesheets {
                global: None,
                layout: Some(hypreact_core::runtime::prepared_layout::PreparedStylesheet {
                    path: "layouts/master-stack/index.css".into(),
                    source:
                        "workspace { display: flex; width: 320px; height: 200px; } #main { width: 200px; }"
                            .into(),
                }),
            },
            ..request_a.clone()
        };

        let response_a = cache.compute_layout_from_request(&request_a).unwrap();
        let response_b = cache.compute_layout_from_request(&request_b).unwrap();

        let width_a = response_a
            .root
            .find_by_window_id(&WindowId::from("win-1"))
            .expect("window node should exist")
            .rect()
            .width;
        let width_b = response_b
            .root
            .find_by_window_id(&WindowId::from("win-1"))
            .expect("window node should exist")
            .rect()
            .width;

        assert_ne!(width_a, width_b);
    }
}
