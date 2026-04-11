use crate::motion::runtime::MotionModifier;
use crate::motion::state::{MotionAnimationSpec, MotionTransitionSpec, MotionValueState};

pub(super) fn ensure_initialized<M: MotionModifier>(
    state: &mut MotionValueState<M::Value>,
    base_value: &M::Value,
    transition_spec: &Option<MotionTransitionSpec>,
    animation_spec: &Option<MotionAnimationSpec<M::Value>>,
) {
    if state.initialized {
        return;
    }

    state.initialized = true;
    state.current_value = base_value.clone();
    state.target_value = base_value.clone();
    state.last_transition_spec = transition_spec.clone();
    state.last_animation_spec = animation_spec.clone();
}
