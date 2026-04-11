use keyframe::ease;
use keyframe::functions::{BezierCurve, EaseIn, EaseInOut, EaseOut, Linear};
use keyframe::mint::Vector2;

use crate::style::{
    MotionEasingKeywordValue, MotionEasingValue, MotionPropertyValue, MotionTimeValue,
    StepPositionValue,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTransition {
    pub property: MotionPropertyValue,
    pub duration: MotionTimeValue,
    pub timing_function: MotionEasingValue,
    pub delay: MotionTimeValue,
}

pub fn expand_transition_lists(
    properties: &[MotionPropertyValue],
    durations: &[MotionTimeValue],
    timing_functions: &[MotionEasingValue],
    delays: &[MotionTimeValue],
) -> Vec<ResolvedTransition> {
    let count = [properties.len(), durations.len(), timing_functions.len(), delays.len()]
        .into_iter()
        .max()
        .unwrap_or(0);

    if count == 0 {
        return Vec::new();
    }

    (0..count)
        .map(|index| ResolvedTransition {
            property: cycle_value(properties, index).cloned().unwrap_or(MotionPropertyValue::All),
            duration: cycle_value(durations, index).copied().unwrap_or(MotionTimeValue(0.0)),
            timing_function: cycle_value(timing_functions, index)
                .cloned()
                .unwrap_or(MotionEasingValue::Keyword(MotionEasingKeywordValue::Ease)),
            delay: cycle_value(delays, index).copied().unwrap_or(MotionTimeValue(0.0)),
        })
        .collect()
}

pub fn sample_easing(easing: &MotionEasingValue, progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    match easing {
        MotionEasingValue::Keyword(keyword) => sample_keyword(*keyword, progress),
        MotionEasingValue::CubicBezier { x1, y1, x2, y2 } => ease(
            BezierCurve::from(Vector2 { x: *x1, y: *y1 }, Vector2 { x: *x2, y: *y2 }),
            0.0f32,
            1.0f32,
            progress,
        ),
        MotionEasingValue::Steps { count, position } => {
            sample_steps((*count).max(1) as f32, *position, progress)
        }
        MotionEasingValue::LinearFunction(stops) => sample_linear_function(stops, progress),
    }
}

fn cycle_value<T>(values: &[T], index: usize) -> Option<&T> {
    if values.is_empty() { None } else { values.get(index % values.len()) }
}

fn sample_keyword(keyword: MotionEasingKeywordValue, progress: f32) -> f32 {
    match keyword {
        MotionEasingKeywordValue::Linear => ease(Linear, 0.0f32, 1.0f32, progress),
        MotionEasingKeywordValue::Ease => ease(
            BezierCurve::from(Vector2 { x: 0.25, y: 0.1 }, Vector2 { x: 0.25, y: 1.0 }),
            0.0f32,
            1.0f32,
            progress,
        ),
        MotionEasingKeywordValue::EaseIn => ease(EaseIn, 0.0f32, 1.0f32, progress),
        MotionEasingKeywordValue::EaseOut => ease(EaseOut, 0.0f32, 1.0f32, progress),
        MotionEasingKeywordValue::EaseInOut => ease(EaseInOut, 0.0f32, 1.0f32, progress),
    }
}

fn sample_steps(count: f32, position: StepPositionValue, progress: f32) -> f32 {
    let mut current_step = (progress * count).floor();

    if matches!(
        position,
        StepPositionValue::Start | StepPositionValue::JumpStart | StepPositionValue::JumpBoth
    ) {
        current_step += 1.0;
    }

    if progress >= 0.0 && current_step < 0.0 {
        current_step = 0.0;
    }

    let jumps = match position {
        StepPositionValue::JumpBoth => count + 1.0,
        StepPositionValue::JumpNone => (count - 1.0).max(1.0),
        _ => count,
    };

    if progress <= 1.0 && current_step > jumps {
        current_step = jumps;
    }

    current_step / jumps
}

fn sample_linear_function(stops: &[crate::style::LinearStopValue], progress: f32) -> f32 {
    match stops {
        [] => progress,
        [single] => single.output,
        _ if progress <= stops[0].input => interpolate_linear_stop(progress, stops[0], stops[1]),
        _ => {
            for pair in stops.windows(2) {
                let [start, end] = pair else {
                    continue;
                };
                if progress <= end.input {
                    return interpolate_linear_stop(progress, *start, *end);
                }
            }
            let last = stops[stops.len() - 1];
            let previous = stops[stops.len() - 2];
            interpolate_linear_stop(progress, previous, last)
        }
    }
}

fn interpolate_linear_stop(
    progress: f32,
    start: crate::style::LinearStopValue,
    end: crate::style::LinearStopValue,
) -> f32 {
    if (end.input - start.input).abs() <= f32::EPSILON {
        return end.output;
    }

    let local = ((progress - start.input) / (end.input - start.input)).clamp(0.0, 1.0);
    start.output + (end.output - start.output) * local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{LinearStopValue, MotionEasingKeywordValue};

    #[test]
    fn expands_transition_lists_with_css_repetition_rules() {
        let transitions = expand_transition_lists(
            &[
                MotionPropertyValue::Named("opacity".into()),
                MotionPropertyValue::Named("transform".into()),
            ],
            &[MotionTimeValue(0.2)],
            &[MotionEasingValue::Keyword(MotionEasingKeywordValue::EaseInOut)],
            &[MotionTimeValue(0.05), MotionTimeValue(0.15)],
        );

        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].duration, MotionTimeValue(0.2));
        assert_eq!(transitions[1].duration, MotionTimeValue(0.2));
        assert_eq!(transitions[1].delay, MotionTimeValue(0.15));
    }

    #[test]
    fn samples_piecewise_linear_timing_functions() {
        let easing = MotionEasingValue::LinearFunction(vec![
            LinearStopValue { input: 0.0, output: 0.0 },
            LinearStopValue { input: 0.25, output: 0.6 },
            LinearStopValue { input: 1.0, output: 1.0 },
        ]);

        let sample = sample_easing(&easing, 0.125);

        assert!(sample > 0.25);
        assert!(sample < 0.4);
    }
}
