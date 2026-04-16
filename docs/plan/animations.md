# Animations Plan

See the existing animation plan content that was previously kept at the repo root.

This document remains a design plan for compositor-delegated animation, not a statement of fully shipped behavior.

The main constraints are:

- CSS is the authored motion surface
- the compositor executes animation timing and rendering
- `tilescript` should not own long-lived animation timeline state for the Hyprland-backed path
- Hyprland-backed support must stay honest about the subset it can actually execute
