use crate::WindowId;
use crate::focus::FocusAxis;
use crate::wm::WindowGeometry;

#[derive(Debug, Clone)]
pub(crate) struct VisualEntry {
    pub(crate) window_id: WindowId,
    pub(crate) geometry: WindowGeometry,
    pub(crate) original_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum VisualChild {
    Scope(VisualScope),
    Window(VisualEntry),
}

#[derive(Debug, Clone)]
pub(crate) struct VisualScope {
    pub(crate) axis: Option<FocusAxis>,
    pub(crate) children: Vec<VisualChild>,
}

/// Infers a purely visual focus hierarchy from final window rectangles.
///
/// Invariants:
/// - `axis.is_some()` only when the scope contains at least two branches.
/// - child order is stable and matches visual order along the selected axis.
/// - leaf scopes have `axis == None` and contain only window children.
pub(crate) fn infer_visual_scope(entries: &[VisualEntry]) -> VisualScope {
    if entries.len() <= 1 {
        return VisualScope {
            axis: None,
            children: entries.iter().cloned().map(VisualChild::Window).collect(),
        };
    }

    let horizontal_bands = cluster_visual_entries(entries, FocusAxis::Horizontal);
    let vertical_bands = cluster_visual_entries(entries, FocusAxis::Vertical);

    let selected_axis = match (split_score(&horizontal_bands), split_score(&vertical_bands)) {
        (Some(horizontal_score), Some(vertical_score)) => {
            if horizontal_score <= vertical_score {
                Some((FocusAxis::Horizontal, horizontal_bands))
            } else {
                Some((FocusAxis::Vertical, vertical_bands))
            }
        }
        (Some(_), None) => Some((FocusAxis::Horizontal, horizontal_bands)),
        (None, Some(_)) => Some((FocusAxis::Vertical, vertical_bands)),
        (None, None) => None,
    };

    let Some((axis, bands)) = selected_axis else {
        let mut ordered_entries = entries.to_vec();
        ordered_entries.sort_by_key(|entry| entry.original_index);
        return VisualScope {
            axis: None,
            children: ordered_entries.into_iter().map(VisualChild::Window).collect(),
        };
    };

    VisualScope {
        axis: Some(axis),
        children: bands
            .into_iter()
            .map(|band| {
                let scope = infer_visual_scope(&band);
                if scope.axis.is_none() && scope.children.len() == 1 {
                    scope.children.into_iter().next().expect("single child")
                } else {
                    VisualChild::Scope(scope)
                }
            })
            .collect(),
    }
}

fn split_score(bands: &[Vec<VisualEntry>]) -> Option<(usize, usize)> {
    if bands.len() <= 1 {
        return None;
    }

    Some((bands.iter().map(|band| band_fragmentation(band)).sum(), bands.len()))
}

fn band_fragmentation(band: &[VisualEntry]) -> usize {
    let mut indices = band.iter().map(|entry| entry.original_index).collect::<Vec<_>>();
    indices.sort_unstable();

    let mut segments: usize = 0;
    let mut previous_index = None;

    for index in indices {
        if previous_index.is_none_or(|previous| index != previous + 1) {
            segments += 1;
        }
        previous_index = Some(index);
    }

    segments.saturating_sub(1)
}

fn cluster_visual_entries(entries: &[VisualEntry], axis: FocusAxis) -> Vec<Vec<VisualEntry>> {
    let mut ordered_entries = entries.to_vec();
    ordered_entries.sort_by_key(|entry| match axis {
        FocusAxis::Horizontal => (entry.geometry.x, entry.geometry.y, entry.original_index as i32),
        FocusAxis::Vertical => (entry.geometry.y, entry.geometry.x, entry.original_index as i32),
    });

    let mut bands: Vec<Vec<VisualEntry>> = Vec::new();
    let mut current_band_end = None;

    for entry in ordered_entries {
        let (start, end) = axis_interval(entry.geometry, axis);

        if current_band_end.is_some_and(|band_end| start < band_end) {
            current_band_end = Some(current_band_end.unwrap().max(end));
            bands.last_mut().expect("existing band").push(entry);
            continue;
        }

        current_band_end = Some(end);
        bands.push(vec![entry]);
    }

    bands
}

fn axis_interval(geometry: WindowGeometry, axis: FocusAxis) -> (i32, i32) {
    match axis {
        FocusAxis::Horizontal => (geometry.x, geometry.x + geometry.width),
        FocusAxis::Vertical => (geometry.y, geometry.y + geometry.height),
    }
}
