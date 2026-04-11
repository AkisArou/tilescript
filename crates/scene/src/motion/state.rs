use std::time::Instant;

use crate::style::{
    AnimationDirectionValue, AnimationFillModeValue, AnimationIterationCountValue,
    AnimationPlayStateValue, MotionEasingValue, TransformValue,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MotionKeyframe<Value> {
    pub offset: f32,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionAnimationSpec<Value> {
    pub name: String,
    pub duration_secs: f32,
    pub delay_secs: f32,
    pub iteration_count: AnimationIterationCountValue,
    pub direction: AnimationDirectionValue,
    pub fill_mode: AnimationFillModeValue,
    pub play_state: AnimationPlayStateValue,
    pub timing_function: MotionEasingValue,
    pub keyframes: Vec<MotionKeyframe<Value>>,
    pub base_value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionTransitionSpec {
    pub duration_secs: f32,
    pub delay_secs: f32,
    pub easing: MotionEasingValue,
}

pub type OpacityKeyframe = MotionKeyframe<f32>;
pub type TransformKeyframe = MotionKeyframe<TransformValue>;
pub type OpacityAnimationSpec = MotionAnimationSpec<f32>;
pub type TransformAnimationSpec = MotionAnimationSpec<TransformValue>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedTransform {
    pub translate_x_px: f32,
    pub translate_y_px: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionContext {
    pub width: f32,
    pub height: f32,
}

impl Default for MotionContext {
    fn default() -> Self {
        Self { width: 0.0, height: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedMotion {
    pub opacity: f32,
    pub transform: ResolvedTransform,
}

impl Default for ResolvedMotion {
    fn default() -> Self {
        Self { opacity: 1.0, transform: ResolvedTransform::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MotionPhaseActivity {
    pub opacity: bool,
    pub transform: bool,
}

impl MotionPhaseActivity {
    pub fn any(self) -> bool {
        self.opacity || self.transform
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedMotionPhase {
    pub motion: ResolvedMotion,
    pub active: MotionPhaseActivity,
}

impl Default for AppliedMotionPhase {
    fn default() -> Self {
        Self { motion: ResolvedMotion::default(), active: MotionPhaseActivity::default() }
    }
}

impl Default for ResolvedTransform {
    fn default() -> Self {
        Self { translate_x_px: 0.0, translate_y_px: 0.0, scale_x: 1.0, scale_y: 1.0 }
    }
}

#[derive(Debug, Clone)]
pub struct ActiveMotionTransition<Value> {
    pub from_value: Value,
    pub to_value: Value,
    pub started_at: Instant,
    pub spec: MotionTransitionSpec,
}

#[derive(Debug, Clone)]
pub struct ActiveMotionAnimation<Value> {
    pub started_at: Instant,
    pub paused_progress: Option<f32>,
    pub spec: MotionAnimationSpec<Value>,
}

#[derive(Debug, Default)]
pub struct MotionValueState<Value> {
    pub initialized: bool,
    pub current_value: Value,
    pub target_value: Value,
    pub last_transition_spec: Option<MotionTransitionSpec>,
    pub last_animation_spec: Option<MotionAnimationSpec<Value>>,
    pub active_transition: Option<ActiveMotionTransition<Value>>,
    pub active_animation: Option<ActiveMotionAnimation<Value>>,
}

pub type OpacityMotionState = MotionValueState<f32>;
pub type TransformMotionState = MotionValueState<TransformValue>;

#[derive(Debug, Default)]
pub struct MotionTrackState {
    pub opacity: OpacityMotionState,
    pub transform: TransformMotionState,
}
