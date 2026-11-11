//! The galaxy's motion state: how lively the account has been lately.
//!
//! A long quiet spell selects [`MotionState::Quiet`]; anything shorter stays
//! [`MotionState::Active`]. Selection is a pure read of the development.

use crate::galaxy::GalaxyDevelopment;

/// Quiet at this many contribution-free days or more; the boundary is part of the spec.
pub const QUIET_DAY_THRESHOLD: u32 = 14;

/// How the galaxy moves: quiet and dim, or active and bright.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MotionState {
    /// Fourteen or more quiet days: dimmer, sparse, slow twinkle, and no flares.
    Quiet,
    /// Recent activity: brighter, dense twinkle, and two seeded flares per loop.
    Active,
}

/// Select the motion state from the development's run of contribution-free days.
pub fn select_motion_state(development: &GalaxyDevelopment) -> MotionState {
    if development.recent_activity.quiet_day_count >= QUIET_DAY_THRESHOLD {
        MotionState::Quiet
    } else {
        MotionState::Active
    }
}
