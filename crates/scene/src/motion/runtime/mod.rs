mod animation;
mod appliers;
mod shared;
mod transition;

use crate::motion::state::{
    MotionContext, MotionPhaseActivity, MotionTrackState, MotionValueState, ResolvedMotion,
};
use crate::{CompiledKeyframeStep, ComputedStyle};

pub(crate) use animation::{AnimationModifierApplier, AnimationModifierApplierHandle};
pub(crate) use appliers::{AnimationAppliers, TransitionAppliers};
pub(crate) use transition::{TransitionModifierApplier, TransitionModifierApplierHandle};

pub trait MotionModifier {
    type Value: Clone + Default + PartialEq;
    type Output: Copy;
    type Context: Copy;

    const PROPERTY_NAME: &'static str;

    fn state(track: &mut MotionTrackState) -> &mut MotionValueState<Self::Value>;
    fn base_value(style: Option<&ComputedStyle>) -> Self::Value;
    fn keyframe_value(step: &CompiledKeyframeStep) -> Option<Self::Value>;
    fn output_from_value(value: &Self::Value, context: Self::Context) -> Self::Output;
    fn value_from_output(output: Self::Output, context: Self::Context) -> Self::Value;
    fn context_from_motion(context: MotionContext) -> Self::Context;
    fn write_output(output: Self::Output, motion: &mut ResolvedMotion);
    fn set_active(active: bool, activity: &mut MotionPhaseActivity);
    fn is_active(activity: MotionPhaseActivity) -> bool;
    fn interpolate_output(
        start: Self::Output,
        end: Self::Output,
        progress: f32,
        context: Self::Context,
    ) -> Self::Output;
}
