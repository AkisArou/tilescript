use crate::motion::runtime::MotionModifier;
use crate::motion::state::{
    MotionContext, MotionPhaseActivity, MotionTrackState, MotionValueState, ResolvedMotion,
};
use crate::{CompiledDeclaration, CompiledKeyframeStep, ComputedStyle};

#[cfg(test)]
use crate::motion::runtime::{AnimationModifierApplier, TransitionModifierApplier};
#[cfg(test)]
use crate::motion::state::ActiveMotionTransition;
#[cfg(test)]
use std::time::Instant;

pub struct OpacityMotion;

impl MotionModifier for OpacityMotion {
    type Value = f32;
    type Output = f32;
    type Context = ();

    const PROPERTY_NAME: &'static str = "opacity";

    fn state(track: &mut MotionTrackState) -> &mut MotionValueState<Self::Value> {
        &mut track.opacity
    }

    fn base_value(style: Option<&ComputedStyle>) -> Self::Value {
        style.and_then(|style| style.opacity).unwrap_or(1.0).clamp(0.0, 1.0)
    }

    fn keyframe_value(step: &CompiledKeyframeStep) -> Option<Self::Value> {
        step.declarations
            .iter()
            .fold(None::<f32>, |current, declaration| match declaration {
                CompiledDeclaration::Opacity(value) => Some(*value),
                _ => current,
            })
            .map(|value| value.clamp(0.0, 1.0))
    }

    fn output_from_value(value: &Self::Value, _context: Self::Context) -> Self::Output {
        *value
    }

    fn value_from_output(output: Self::Output, _context: Self::Context) -> Self::Value {
        output
    }

    fn context_from_motion(_context: MotionContext) -> Self::Context {}

    fn write_output(output: Self::Output, motion: &mut ResolvedMotion) {
        motion.opacity = output;
    }

    fn set_active(active: bool, activity: &mut MotionPhaseActivity) {
        activity.opacity = active;
    }

    fn is_active(activity: MotionPhaseActivity) -> bool {
        activity.opacity
    }

    fn interpolate_output(
        start: Self::Output,
        end: Self::Output,
        progress: f32,
        _context: Self::Context,
    ) -> Self::Output {
        start + (end - start) * progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::state::MotionTransitionSpec;
    use crate::style::{MotionEasingKeywordValue, MotionEasingValue};
    use crate::{CompiledKeyframeStep, CompiledKeyframesRule};

    #[test]
    fn opacity_keyframes_fill_missing_endpoints_from_base_opacity() {
        let rule = CompiledKeyframesRule {
            name: "fade".into(),
            steps: vec![
                CompiledKeyframeStep {
                    offset: 0.4,
                    declarations: vec![CompiledDeclaration::Opacity(0.25)],
                },
                CompiledKeyframeStep {
                    offset: 0.8,
                    declarations: vec![CompiledDeclaration::Opacity(0.8)],
                },
            ],
        };

        let frames = AnimationModifierApplier::<OpacityMotion>::new()
            .keyframes_from_rule(&rule, &1.0)
            .unwrap();

        assert_eq!(frames.first().unwrap().offset, 0.0);
        assert_eq!(frames.first().unwrap().value, 1.0);
        assert_eq!(frames.last().unwrap().offset, 1.0);
        assert_eq!(frames.last().unwrap().value, 1.0);
    }

    #[test]
    fn transition_sampling_reaches_target_after_duration() {
        let started_at = Instant::now();
        let transition = ActiveMotionTransition {
            from_value: 0.2,
            to_value: 0.8,
            started_at,
            spec: MotionTransitionSpec {
                duration_secs: 0.2,
                delay_secs: 0.0,
                easing: MotionEasingValue::Keyword(MotionEasingKeywordValue::Linear),
            },
        };

        let (value, active) = TransitionModifierApplier::<OpacityMotion>::new().sample_transition(
            &transition,
            MotionContext::default(),
            started_at + std::time::Duration::from_millis(200),
        );

        assert!(!active);
        assert!((value - 0.8).abs() < 0.0001);
    }
}
