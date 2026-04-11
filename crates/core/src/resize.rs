use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DEFAULT_SPLIT_WEIGHT: u16 = 1;
pub const MIN_SPLIT_WEIGHT: u16 = 1;
pub const MAX_SPLIT_WEIGHT: u16 = 24;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutAdjustmentState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub split_weights_by_node_id: BTreeMap<String, Vec<u16>>,
}

impl LayoutAdjustmentState {
    pub fn is_empty(&self) -> bool {
        self.split_weights_by_node_id.is_empty()
    }
}

pub fn normalize_split_weights(
    current: Option<&[u16]>,
    child_count: usize,
    default_weights: &[u16],
) -> Vec<u16> {
    (0..child_count)
        .map(|index| {
            current
                .and_then(|weights| weights.get(index).copied())
                .or_else(|| default_weights.get(index).copied())
                .or_else(|| default_weights.last().copied())
                .unwrap_or(DEFAULT_SPLIT_WEIGHT)
                .clamp(MIN_SPLIT_WEIGHT, MAX_SPLIT_WEIGHT)
        })
        .collect()
}

pub fn resize_split_weights(
    current: Option<&[u16]>,
    child_count: usize,
    default_weights: &[u16],
    grow_index: usize,
    shrink_index: usize,
) -> Option<Vec<u16>> {
    if child_count < 2
        || grow_index >= child_count
        || shrink_index >= child_count
        || grow_index == shrink_index
    {
        return None;
    }

    let mut weights = normalize_split_weights(current, child_count, default_weights);
    if weights[grow_index] >= MAX_SPLIT_WEIGHT || weights[shrink_index] <= MIN_SPLIT_WEIGHT {
        return None;
    }

    weights[grow_index] += 1;
    weights[shrink_index] -= 1;
    Some(weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_split_weights_pads_with_defaults() {
        let weights = normalize_split_weights(Some(&[6]), 3, &[12, 8, 8]);

        assert_eq!(weights, vec![6, 8, 8]);
    }

    #[test]
    fn resize_split_weights_grows_and_shrinks_neighbors() {
        let weights = resize_split_weights(None, 2, &[12, 8], 0, 1);

        assert_eq!(weights, Some(vec![13, 7]));
    }

    #[test]
    fn resize_split_weights_stops_at_minimum() {
        let weights = resize_split_weights(Some(&[24, 1]), 2, &[12, 8], 0, 1);

        assert_eq!(weights, None);
    }
}
