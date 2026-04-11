use std::marker::PhantomData;
use std::time::Instant;

use crate::motion::easing::sample_easing;
use crate::motion::runtime::MotionModifier;
use crate::motion::runtime::shared::ensure_initialized;
use crate::motion::state::{
    ActiveMotionAnimation, AppliedMotionPhase, MotionAnimationSpec, MotionContext, MotionKeyframe,
    MotionTrackState,
};
use crate::style::{
    AnimationDirectionValue, AnimationFillModeValue, AnimationIterationCountValue,
    AnimationPlayStateValue, MotionEasingKeywordValue, MotionEasingValue, MotionTimeValue,
};
use crate::{CompiledKeyframesRule, ComputedStyle};

pub(crate) trait AnimationModifierApplierHandle {
    fn apply(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        keyframes: &[CompiledKeyframesRule],
        context: MotionContext,
        now: Instant,
        applied: &mut AppliedMotionPhase,
    );
}

pub(crate) struct AnimationModifierApplier<M>(PhantomData<M>);

impl<M> AnimationModifierApplier<M> {
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<M: MotionModifier> AnimationModifierApplierHandle for AnimationModifierApplier<M> {
    fn apply(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        keyframes: &[CompiledKeyframesRule],
        context: MotionContext,
        now: Instant,
        applied: &mut AppliedMotionPhase,
    ) {
        let (output, active) = self.apply_modifier(track, style, keyframes, context, now);
        M::write_output(output, &mut applied.motion);
        M::set_active(active, &mut applied.active);
    }
}

impl<M: MotionModifier> AnimationModifierApplier<M> {
    pub(crate) fn applies_to_style(
        &self,
        style: Option<&ComputedStyle>,
        keyframes: &[CompiledKeyframesRule],
    ) -> bool {
        let base_value = M::base_value(style);
        style.and_then(|style| self.extract_animation_spec(style, keyframes, &base_value)).is_some()
    }

    pub(crate) fn apply_modifier(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        keyframes: &[CompiledKeyframesRule],
        context: MotionContext,
        now: Instant,
    ) -> (M::Output, bool) {
        let state = M::state(track);
        let base_value = M::base_value(style);
        let animation_spec =
            style.and_then(|style| self.extract_animation_spec(style, keyframes, &base_value));

        self.apply_state(state, &base_value, animation_spec, M::context_from_motion(context), now)
    }

    #[cfg(test)]
    pub(crate) fn keyframes_from_rule(
        &self,
        rule: &CompiledKeyframesRule,
        base_value: &M::Value,
    ) -> Option<Vec<MotionKeyframe<M::Value>>> {
        self.collect_keyframes_from_rule(rule, base_value)
    }

    fn extract_animation_spec(
        &self,
        style: &ComputedStyle,
        keyframes: &[CompiledKeyframesRule],
        base_value: &M::Value,
    ) -> Option<MotionAnimationSpec<M::Value>> {
        let names = style.animation_name.as_deref().unwrap_or(&[]);
        if names.is_empty() {
            return None;
        }

        let count = [
            names.len(),
            style.animation_duration.as_ref().map_or(0, Vec::len),
            style.animation_timing_function.as_ref().map_or(0, Vec::len),
            style.animation_delay.as_ref().map_or(0, Vec::len),
            style.animation_iteration_count.as_ref().map_or(0, Vec::len),
            style.animation_direction.as_ref().map_or(0, Vec::len),
            style.animation_fill_mode.as_ref().map_or(0, Vec::len),
            style.animation_play_state.as_ref().map_or(0, Vec::len),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);

        (0..count).rev().find_map(|index| {
            let name = cycle_value(names, index)?.clone();
            if name.eq_ignore_ascii_case("none") {
                return None;
            }

            let duration_secs =
                cycle_value(style.animation_duration.as_deref().unwrap_or(&[]), index)
                    .copied()
                    .unwrap_or(MotionTimeValue(0.0))
                    .0
                    .max(0.0);
            if duration_secs <= 0.0 {
                return None;
            }

            let keyframe_rule = keyframes.iter().find(|rule| rule.name == name)?;
            let keyframes = self.collect_keyframes_from_rule(keyframe_rule, base_value)?;

            Some(MotionAnimationSpec {
                name,
                duration_secs,
                delay_secs: cycle_value(style.animation_delay.as_deref().unwrap_or(&[]), index)
                    .copied()
                    .unwrap_or(MotionTimeValue(0.0))
                    .0,
                iteration_count: cycle_value(
                    style.animation_iteration_count.as_deref().unwrap_or(&[]),
                    index,
                )
                .cloned()
                .unwrap_or(AnimationIterationCountValue::Number(1.0)),
                direction: cycle_value(style.animation_direction.as_deref().unwrap_or(&[]), index)
                    .copied()
                    .unwrap_or(AnimationDirectionValue::Normal),
                fill_mode: cycle_value(style.animation_fill_mode.as_deref().unwrap_or(&[]), index)
                    .copied()
                    .unwrap_or(AnimationFillModeValue::None),
                play_state: cycle_value(
                    style.animation_play_state.as_deref().unwrap_or(&[]),
                    index,
                )
                .copied()
                .unwrap_or(AnimationPlayStateValue::Running),
                timing_function: cycle_value(
                    style.animation_timing_function.as_deref().unwrap_or(&[]),
                    index,
                )
                .cloned()
                .unwrap_or(MotionEasingValue::Keyword(MotionEasingKeywordValue::Ease)),
                keyframes,
                base_value: base_value.clone(),
            })
        })
    }

    fn collect_keyframes_from_rule(
        &self,
        rule: &CompiledKeyframesRule,
        base_value: &M::Value,
    ) -> Option<Vec<MotionKeyframe<M::Value>>> {
        let mut frames = rule
            .steps
            .iter()
            .filter_map(|step| {
                M::keyframe_value(step)
                    .map(|value| MotionKeyframe { offset: step.offset.clamp(0.0, 1.0), value })
            })
            .collect::<Vec<_>>();

        if frames.is_empty() {
            return None;
        }

        if !frames.iter().any(|frame| frame.offset <= 0.0) {
            frames.push(MotionKeyframe { offset: 0.0, value: base_value.clone() });
        }
        if !frames.iter().any(|frame| frame.offset >= 1.0) {
            frames.push(MotionKeyframe { offset: 1.0, value: base_value.clone() });
        }

        frames.sort_by(|left, right| {
            left.offset.partial_cmp(&right.offset).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut deduped: Vec<MotionKeyframe<M::Value>> = Vec::with_capacity(frames.len());
        for frame in frames {
            if let Some(last) = deduped.last_mut()
                && (last.offset - frame.offset).abs() <= f32::EPSILON
            {
                *last = frame;
            } else {
                deduped.push(frame);
            }
        }

        Some(deduped)
    }

    fn paused_progress(&self, spec: &MotionAnimationSpec<M::Value>) -> Option<f32> {
        match spec.play_state {
            AnimationPlayStateValue::Paused => Some(self.initial_progress(spec)),
            AnimationPlayStateValue::Running => None,
        }
    }

    fn apply_state(
        &self,
        state: &mut crate::motion::state::MotionValueState<M::Value>,
        base_value: &M::Value,
        animation_spec: Option<MotionAnimationSpec<M::Value>>,
        context: M::Context,
        now: Instant,
    ) -> (M::Output, bool) {
        ensure_initialized::<M>(
            state,
            base_value,
            &state.last_transition_spec.clone(),
            &animation_spec,
        );

        if state.last_animation_spec != animation_spec {
            state.active_animation = animation_spec.clone().map(|spec| ActiveMotionAnimation {
                paused_progress: self.paused_progress(&spec),
                started_at: now,
                spec,
            });
        }

        state.last_animation_spec = animation_spec;

        if let Some(animation) = state.active_animation.as_mut() {
            let (value, keep, active) = self.sample_animation(animation, context, now);
            if !keep {
                state.active_animation = None;
            }
            state.current_value = M::value_from_output(value, context);
            return (value, active);
        }

        let current = M::output_from_value(base_value, context);
        state.current_value = base_value.clone();
        (current, false)
    }

    fn initial_progress(&self, spec: &MotionAnimationSpec<M::Value>) -> f32 {
        if spec.duration_secs <= 0.0 {
            0.0
        } else if spec.delay_secs < 0.0 {
            (-spec.delay_secs / spec.duration_secs).max(0.0)
        } else {
            0.0
        }
    }

    fn sample_animation(
        &self,
        animation: &ActiveMotionAnimation<M::Value>,
        context: M::Context,
        now: Instant,
    ) -> (M::Output, bool, bool) {
        let spec = &animation.spec;
        let overall_progress = animation.paused_progress.unwrap_or_else(|| {
            let elapsed = now.saturating_duration_since(animation.started_at).as_secs_f32();
            if spec.duration_secs <= 0.0 {
                0.0
            } else {
                (elapsed - spec.delay_secs) / spec.duration_secs
            }
        });

        let is_running = matches!(spec.play_state, AnimationPlayStateValue::Running)
            && animation.paused_progress.is_none();

        if overall_progress < 0.0 {
            let value = if matches!(
                spec.fill_mode,
                AnimationFillModeValue::Backwards | AnimationFillModeValue::Both
            ) {
                self.sample_keyframes(spec, context, 0.0)
            } else {
                M::output_from_value(&spec.base_value, context)
            };
            return (value, true, is_running);
        }

        let finite_iterations = match spec.iteration_count {
            AnimationIterationCountValue::Number(count) => Some(count.max(0.0)),
            AnimationIterationCountValue::Infinite => None,
        };

        if let Some(iterations) = finite_iterations
            && overall_progress >= iterations
        {
            let value = if matches!(
                spec.fill_mode,
                AnimationFillModeValue::Forwards | AnimationFillModeValue::Both
            ) {
                self.sample_keyframes(spec, context, self.terminal_progress(spec, iterations))
            } else {
                M::output_from_value(&spec.base_value, context)
            };
            let keep = matches!(
                spec.fill_mode,
                AnimationFillModeValue::Forwards | AnimationFillModeValue::Both
            ) || matches!(spec.play_state, AnimationPlayStateValue::Paused);
            return (value, keep, false);
        }

        let value = self.sample_keyframes(
            spec,
            context,
            self.effective_progress(spec, overall_progress.max(0.0)),
        );
        (value, true, is_running)
    }

    fn terminal_progress(&self, spec: &MotionAnimationSpec<M::Value>, iterations: f32) -> f32 {
        if iterations <= 0.0 {
            return 0.0;
        }

        self.effective_progress(spec, (iterations - f32::EPSILON).max(0.0))
    }

    fn effective_progress(
        &self,
        spec: &MotionAnimationSpec<M::Value>,
        overall_progress: f32,
    ) -> f32 {
        let cycle_index = overall_progress.floor() as u32;
        let mut cycle_progress = overall_progress.fract();
        if cycle_progress.abs() <= f32::EPSILON && overall_progress > 0.0 {
            cycle_progress = 1.0;
        }

        let reverse = match spec.direction {
            AnimationDirectionValue::Normal => false,
            AnimationDirectionValue::Reverse => true,
            AnimationDirectionValue::Alternate => cycle_index % 2 == 1,
            AnimationDirectionValue::AlternateReverse => cycle_index % 2 == 0,
        };

        if reverse { 1.0 - cycle_progress } else { cycle_progress }
    }

    fn sample_keyframes(
        &self,
        spec: &MotionAnimationSpec<M::Value>,
        context: M::Context,
        progress: f32,
    ) -> M::Output {
        match spec.keyframes.as_slice() {
            [] => M::output_from_value(&spec.base_value, context),
            [single] => M::output_from_value(&single.value, context),
            frames => {
                let progress = progress.clamp(0.0, 1.0);
                if progress <= frames[0].offset {
                    return M::output_from_value(&frames[0].value, context);
                }

                for pair in frames.windows(2) {
                    let [start, end] = pair else {
                        continue;
                    };
                    if progress <= end.offset {
                        if (end.offset - start.offset).abs() <= f32::EPSILON {
                            return M::output_from_value(&end.value, context);
                        }
                        let local = (progress - start.offset) / (end.offset - start.offset);
                        let eased = sample_easing(&spec.timing_function, local);
                        let start = M::output_from_value(&start.value, context);
                        let end = M::output_from_value(&end.value, context);
                        return M::interpolate_output(start, end, eased, context);
                    }
                }

                frames
                    .last()
                    .map(|frame| M::output_from_value(&frame.value, context))
                    .unwrap_or_else(|| M::output_from_value(&spec.base_value, context))
            }
        }
    }
}

fn cycle_value<T>(values: &[T], index: usize) -> Option<&T> {
    if values.is_empty() { None } else { values.get(index % values.len()) }
}
