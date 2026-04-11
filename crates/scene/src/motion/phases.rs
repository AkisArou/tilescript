use std::time::Instant;

use crate::motion::runtime::{AnimationAppliers, TransitionAppliers};
use crate::motion::state::{
    AppliedMotionPhase, MotionContext, MotionPhaseActivity, MotionTrackState,
};
use crate::{CompiledKeyframesRule, ComputedStyle};

pub struct Animation<'a> {
    style: Option<&'a ComputedStyle>,
    keyframes: &'a [CompiledKeyframesRule],
    appliers: AnimationAppliers,
}

impl<'a> Animation<'a> {
    pub fn new(style: Option<&'a ComputedStyle>, keyframes: &'a [CompiledKeyframesRule]) -> Self {
        Self { style, keyframes, appliers: AnimationAppliers::new(style, keyframes) }
    }

    pub fn apply(
        &self,
        track: &mut MotionTrackState,
        context: MotionContext,
        now: Instant,
    ) -> AppliedMotionPhase {
        let mut applied = AppliedMotionPhase::default();
        self.appliers.apply(track, self.style, self.keyframes, context, now, &mut applied);
        applied
    }
}

pub struct Transition<'a> {
    style: Option<&'a ComputedStyle>,
    appliers: TransitionAppliers,
}

impl<'a> Transition<'a> {
    pub fn new(style: Option<&'a ComputedStyle>) -> Self {
        Self { style, appliers: TransitionAppliers::new(style) }
    }

    pub fn apply(
        &self,
        track: &mut MotionTrackState,
        context: MotionContext,
        animation: MotionPhaseActivity,
        now: Instant,
    ) -> AppliedMotionPhase {
        let mut applied = AppliedMotionPhase::default();
        self.appliers.apply(track, self.style, context, animation, now, &mut applied);
        applied
    }
}
