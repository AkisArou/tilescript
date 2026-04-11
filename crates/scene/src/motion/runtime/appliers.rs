use std::time::Instant;

use crate::motion::modifiers::{OpacityMotion, TransformMotion};
use crate::motion::runtime::{
    AnimationModifierApplier, AnimationModifierApplierHandle, MotionModifier,
    TransitionModifierApplier, TransitionModifierApplierHandle,
};
use crate::motion::state::{
    AppliedMotionPhase, MotionContext, MotionPhaseActivity, MotionTrackState,
};
use crate::{CompiledKeyframesRule, ComputedStyle};

type AnimationApplier = Box<dyn AnimationModifierApplierHandle>;
type TransitionApplier = Box<dyn TransitionModifierApplierHandle>;

pub(crate) struct AnimationAppliers(Vec<AnimationApplier>);

impl AnimationAppliers {
    pub(crate) fn new(style: Option<&ComputedStyle>, keyframes: &[CompiledKeyframesRule]) -> Self {
        let mut appliers = Vec::new();

        push_animation_applier::<OpacityMotion>(&mut appliers, style, keyframes);
        push_animation_applier::<TransformMotion>(&mut appliers, style, keyframes);

        Self(appliers)
    }

    pub(crate) fn apply(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        keyframes: &[CompiledKeyframesRule],
        context: MotionContext,
        now: Instant,
        applied: &mut AppliedMotionPhase,
    ) {
        for applier in &self.0 {
            applier.apply(track, style, keyframes, context, now, applied);
        }
    }
}

pub(crate) struct TransitionAppliers(Vec<TransitionApplier>);

impl TransitionAppliers {
    pub(crate) fn new(style: Option<&ComputedStyle>) -> Self {
        let mut appliers = Vec::new();

        push_transition_applier::<OpacityMotion>(&mut appliers, style);
        push_transition_applier::<TransformMotion>(&mut appliers, style);

        Self(appliers)
    }

    pub(crate) fn apply(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        context: MotionContext,
        animation: MotionPhaseActivity,
        now: Instant,
        applied: &mut AppliedMotionPhase,
    ) {
        for applier in &self.0 {
            applier.apply(track, style, context, animation, now, applied);
        }
    }
}

fn push_animation_applier<M: MotionModifier + 'static>(
    appliers: &mut Vec<AnimationApplier>,
    style: Option<&ComputedStyle>,
    keyframes: &[CompiledKeyframesRule],
) {
    let applier = AnimationModifierApplier::<M>::new();
    if applier.applies_to_style(style, keyframes) {
        appliers.push(Box::new(applier) as AnimationApplier);
    }
}

fn push_transition_applier<M: MotionModifier + 'static>(
    appliers: &mut Vec<TransitionApplier>,
    style: Option<&ComputedStyle>,
) {
    let applier = TransitionModifierApplier::<M>::new();
    if applier.applies_to_style(style) {
        appliers.push(Box::new(applier) as TransitionApplier);
    }
}
