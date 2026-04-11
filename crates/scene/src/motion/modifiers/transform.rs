use crate::motion::runtime::MotionModifier;
use crate::motion::state::{
    MotionContext, MotionPhaseActivity, MotionTrackState, MotionValueState, ResolvedMotion,
    ResolvedTransform,
};
use crate::style::{
    LengthPercentage, ScaleTransformValue, TransformOperationValue, TransformValue,
    TranslateTransformValue,
};
use crate::{CompiledDeclaration, CompiledKeyframeStep, ComputedStyle};

#[cfg(test)]
use crate::CompiledKeyframesRule;
#[cfg(test)]
use crate::motion::runtime::{AnimationModifierApplier, TransitionModifierApplier};
#[cfg(test)]
use crate::motion::state::ActiveMotionTransition;
#[cfg(test)]
use std::time::Instant;

pub struct TransformMotion;

impl MotionModifier for TransformMotion {
    type Value = TransformValue;
    type Output = ResolvedTransform;
    type Context = MotionContext;

    const PROPERTY_NAME: &'static str = "transform";

    fn state(track: &mut MotionTrackState) -> &mut MotionValueState<Self::Value> {
        &mut track.transform
    }

    fn base_value(style: Option<&ComputedStyle>) -> Self::Value {
        style.and_then(|style| style.transform.clone()).unwrap_or_default()
    }

    fn keyframe_value(step: &CompiledKeyframeStep) -> Option<Self::Value> {
        step.declarations.iter().find_map(|declaration| match declaration {
            CompiledDeclaration::Transform(value) => Some(value.clone()),
            _ => None,
        })
    }

    fn output_from_value(value: &Self::Value, context: Self::Context) -> Self::Output {
        resolve_value(value, context)
    }

    fn value_from_output(output: Self::Output, _context: Self::Context) -> Self::Value {
        transform_value_from_resolved(output)
    }

    fn context_from_motion(context: MotionContext) -> Self::Context {
        context
    }

    fn write_output(output: Self::Output, motion: &mut ResolvedMotion) {
        motion.transform = output;
    }

    fn set_active(active: bool, activity: &mut MotionPhaseActivity) {
        activity.transform = active;
    }

    fn is_active(activity: MotionPhaseActivity) -> bool {
        activity.transform
    }

    fn interpolate_output(
        start: Self::Output,
        end: Self::Output,
        progress: f32,
        _context: Self::Context,
    ) -> Self::Output {
        ResolvedTransform {
            translate_x_px: start.translate_x_px
                + (end.translate_x_px - start.translate_x_px) * progress,
            translate_y_px: start.translate_y_px
                + (end.translate_y_px - start.translate_y_px) * progress,
            scale_x: start.scale_x + (end.scale_x - start.scale_x) * progress,
            scale_y: start.scale_y + (end.scale_y - start.scale_y) * progress,
        }
    }
}

fn resolve_value(transform: &TransformValue, context: MotionContext) -> ResolvedTransform {
    let mut resolved = ResolvedTransform::default();

    for operation in &transform.operations {
        match operation {
            TransformOperationValue::Translate(TranslateTransformValue { x, y }) => {
                resolved.translate_x_px += resolve_length_percentage(*x, context.width);
                resolved.translate_y_px += resolve_length_percentage(*y, context.height);
            }
            TransformOperationValue::Scale(ScaleTransformValue { x, y }) => {
                resolved.scale_x *= *x;
                resolved.scale_y *= *y;
            }
        }
    }

    resolved
}

fn transform_value_from_resolved(transform: ResolvedTransform) -> TransformValue {
    let mut operations = Vec::new();
    if transform.translate_x_px.abs() > f32::EPSILON
        || transform.translate_y_px.abs() > f32::EPSILON
    {
        operations.push(TransformOperationValue::Translate(TranslateTransformValue {
            x: LengthPercentage::Px(transform.translate_x_px),
            y: LengthPercentage::Px(transform.translate_y_px),
        }));
    }
    if (transform.scale_x - 1.0).abs() > f32::EPSILON
        || (transform.scale_y - 1.0).abs() > f32::EPSILON
    {
        operations.push(TransformOperationValue::Scale(ScaleTransformValue {
            x: transform.scale_x,
            y: transform.scale_y,
        }));
    }

    TransformValue { operations }
}

fn resolve_length_percentage(value: LengthPercentage, basis: f32) -> f32 {
    match value {
        LengthPercentage::Px(px) => px,
        LengthPercentage::Percent(percent) => basis * (percent / 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::state::MotionTransitionSpec;
    use crate::style::{MotionEasingKeywordValue, MotionEasingValue};

    #[test]
    fn transform_keyframes_fill_missing_endpoints_from_base_transform() {
        let rule = CompiledKeyframesRule {
            name: "slide".into(),
            steps: vec![CompiledKeyframeStep {
                offset: 0.5,
                declarations: vec![CompiledDeclaration::Transform(TransformValue {
                    operations: vec![TransformOperationValue::Translate(TranslateTransformValue {
                        x: LengthPercentage::Percent(100.0),
                        y: LengthPercentage::Px(0.0),
                    })],
                })],
            }],
        };

        let frames = AnimationModifierApplier::<TransformMotion>::new()
            .keyframes_from_rule(&rule, &TransformValue::default())
            .unwrap();

        assert_eq!(frames.first().unwrap().offset, 0.0);
        assert!(frames.first().unwrap().value.operations.is_empty());
        assert_eq!(frames.last().unwrap().offset, 1.0);
        assert!(frames.last().unwrap().value.operations.is_empty());
    }

    #[test]
    fn transform_transition_resolves_percent_translation_to_pixels() {
        let started_at = Instant::now();
        let transition = ActiveMotionTransition {
            from_value: TransformValue::default(),
            to_value: TransformValue {
                operations: vec![
                    TransformOperationValue::Translate(TranslateTransformValue {
                        x: LengthPercentage::Percent(100.0),
                        y: LengthPercentage::Px(18.0),
                    }),
                    TransformOperationValue::Scale(ScaleTransformValue { x: 0.8, y: 0.8 }),
                ],
            },
            started_at,
            spec: MotionTransitionSpec {
                duration_secs: 0.2,
                delay_secs: 0.0,
                easing: MotionEasingValue::Keyword(MotionEasingKeywordValue::Linear),
            },
        };

        let (value, active) = TransitionModifierApplier::<TransformMotion>::new()
            .sample_transition(
                &transition,
                MotionContext { width: 320.0, height: 120.0 },
                started_at + std::time::Duration::from_millis(200),
            );

        assert!(!active);
        assert!((value.translate_x_px - 320.0).abs() < 0.001);
        assert!((value.translate_y_px - 18.0).abs() < 0.001);
        assert!((value.scale_x - 0.8).abs() < 0.001);
        assert!((value.scale_y - 0.8).abs() < 0.001);
    }
}
