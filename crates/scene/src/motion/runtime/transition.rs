use std::marker::PhantomData;
use std::time::Instant;

use crate::ComputedStyle;
use crate::motion::easing::{expand_transition_lists, sample_easing};
use crate::motion::runtime::MotionModifier;
use crate::motion::runtime::shared::ensure_initialized;
use crate::motion::state::{
    ActiveMotionTransition, AppliedMotionPhase, MotionContext, MotionPhaseActivity,
    MotionTrackState, MotionTransitionSpec,
};
use crate::style::MotionPropertyValue;

pub(crate) trait TransitionModifierApplierHandle {
    fn apply(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        context: MotionContext,
        animation: MotionPhaseActivity,
        now: Instant,
        applied: &mut AppliedMotionPhase,
    );
}

pub(crate) struct TransitionModifierApplier<M>(PhantomData<M>);

impl<M> TransitionModifierApplier<M> {
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<M: MotionModifier> TransitionModifierApplierHandle for TransitionModifierApplier<M> {
    fn apply(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        context: MotionContext,
        animation: MotionPhaseActivity,
        now: Instant,
        applied: &mut AppliedMotionPhase,
    ) {
        let (output, active) = self.apply_modifier(track, style, context, animation, now);
        M::write_output(output, &mut applied.motion);
        M::set_active(active, &mut applied.active);
    }
}

impl<M: MotionModifier> TransitionModifierApplier<M> {
    pub(crate) fn applies_to_style(&self, style: Option<&ComputedStyle>) -> bool {
        style.and_then(|style| self.extract_transition_spec(style)).is_some()
    }

    pub(crate) fn apply_modifier(
        &self,
        track: &mut MotionTrackState,
        style: Option<&ComputedStyle>,
        context: MotionContext,
        animation: MotionPhaseActivity,
        now: Instant,
    ) -> (M::Output, bool) {
        let state = M::state(track);
        let base_value = M::base_value(style);
        let transition_spec = style.and_then(|style| self.extract_transition_spec(style));

        self.apply_state(
            state,
            &base_value,
            transition_spec,
            M::is_active(animation),
            M::context_from_motion(context),
            now,
        )
    }

    #[cfg(test)]
    pub(crate) fn sample_transition(
        &self,
        transition: &ActiveMotionTransition<M::Value>,
        context: MotionContext,
        now: Instant,
    ) -> (M::Output, bool) {
        self.sample_transition_value(transition, M::context_from_motion(context), now)
    }

    fn extract_transition_spec(&self, style: &ComputedStyle) -> Option<MotionTransitionSpec> {
        let transitions = expand_transition_lists(
            style.transition_property.as_deref().unwrap_or(&[]),
            style.transition_duration.as_deref().unwrap_or(&[]),
            style.transition_timing_function.as_deref().unwrap_or(&[]),
            style.transition_delay.as_deref().unwrap_or(&[]),
        );

        transitions
            .into_iter()
            .rev()
            .find(|transition| match &transition.property {
                MotionPropertyValue::All => true,
                MotionPropertyValue::Named(name) => name.eq_ignore_ascii_case(M::PROPERTY_NAME),
            })
            .map(|transition| MotionTransitionSpec {
                duration_secs: transition.duration.0.max(0.0),
                delay_secs: transition.delay.0,
                easing: transition.timing_function,
            })
    }

    fn apply_state(
        &self,
        state: &mut crate::motion::state::MotionValueState<M::Value>,
        base_value: &M::Value,
        transition_spec: Option<MotionTransitionSpec>,
        animation_active: bool,
        context: M::Context,
        now: Instant,
    ) -> (M::Output, bool) {
        let last_animation_spec = state.last_animation_spec.clone();
        ensure_initialized::<M>(state, base_value, &transition_spec, &last_animation_spec);

        let (current_before_update, active_before_update) =
            if let Some(transition) = state.active_transition.as_mut() {
                self.sample_transition_value(transition, context, now)
            } else {
                (M::output_from_value(&state.target_value, context), false)
            };

        if animation_active {
            state.active_transition = None;
            state.target_value = base_value.clone();
            state.last_transition_spec = transition_spec;
            return (current_before_update, false);
        }

        state.current_value = M::value_from_output(current_before_update, context);

        if state.target_value != *base_value {
            state.active_transition = transition_spec.clone().and_then(|spec| {
                (spec.duration_secs > 0.0).then_some(ActiveMotionTransition {
                    from_value: state.current_value.clone(),
                    to_value: base_value.clone(),
                    started_at: now,
                    spec,
                })
            });

            if state.active_transition.is_none() {
                state.current_value = base_value.clone();
            }
        } else if !active_before_update {
            state.current_value = base_value.clone();
        }

        state.target_value = base_value.clone();
        state.last_transition_spec = transition_spec;

        if let Some(transition) = state.active_transition.as_mut() {
            let (value, active) = self.sample_transition_value(transition, context, now);
            if !active {
                state.active_transition = None;
            }
            state.current_value = M::value_from_output(value, context);
            return (value, active);
        }

        let current = M::output_from_value(&state.target_value, context);
        state.current_value = base_value.clone();
        (current, false)
    }

    fn sample_transition_value(
        &self,
        transition: &ActiveMotionTransition<M::Value>,
        context: M::Context,
        now: Instant,
    ) -> (M::Output, bool) {
        let elapsed = now.saturating_duration_since(transition.started_at).as_secs_f32();
        if elapsed < transition.spec.delay_secs {
            return (M::output_from_value(&transition.from_value, context), true);
        }

        if transition.spec.duration_secs <= 0.0 {
            return (M::output_from_value(&transition.to_value, context), false);
        }

        let progress = ((elapsed - transition.spec.delay_secs) / transition.spec.duration_secs)
            .clamp(0.0, 1.0);
        let eased = sample_easing(&transition.spec.easing, progress);
        let from = M::output_from_value(&transition.from_value, context);
        let to = M::output_from_value(&transition.to_value, context);
        (M::interpolate_output(from, to, eased, context), progress < 1.0)
    }
}
