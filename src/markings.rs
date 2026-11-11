//! How the account's weekly streaks mark the galaxy.
//!
//! A running streak lights a comet trail along one spiral arm; the longest streak
//! raises the seeded star count. Both are pure functions of
//! [`GalaxyDevelopment::streaks`], so identical developments mark identical galaxies.
//! This module reads no clock and performs no I/O.

use crate::galaxy::{GalaxyDevelopment, RecentStreak};

/// A running streak lights the comet trail to its full length after this many weeks.
pub const TRAIL_FULL_WEEKS: usize = 13;
/// The longest streak raises the star count to its full gain after this many weeks.
pub const STAR_DENSITY_FULL_WEEKS: usize = 52;
/// Seeded stars every galaxy draws before the longest-streak gain.
pub const BASE_STAR_COUNT: usize = 160;
/// Extra seeded stars a full longest-streak gain adds.
pub const EXTRA_STAR_COUNT: usize = 160;

/// Streak marks the renderer draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreakMarks {
    /// Share of the arm the comet trail covers, from zero through one. Zero means no
    /// trail.
    pub trail_length: f64,
    /// Comet trail brightness, from zero through one. Zero whenever there is no trail.
    pub trail_strength: f64,
    /// Share of [`EXTRA_STAR_COUNT`] the longest streak adds, from zero through one.
    pub star_density_gain: f64,
}

/// Derive the marks from a development's streaks.
///
/// Only a still-running [`RecentStreak::Current`] lights the comet trail: an ended
/// streak has no comet because it is no longer running. Its weeks remain in
/// [`GalaxyDevelopment::streaks.longest`] when they are the record, so they can still
/// raise the star density. No recent streak means no trail and base density. Both
/// mappings are monotonic and capped; the caller supplies the development, so no clock
/// is read.
pub fn calculate_streak_marks(development: &GalaxyDevelopment) -> StreakMarks {
    let current_week_count = match &development.streaks.recent {
        Some(RecentStreak::Current(weeks)) => weeks.len(),
        Some(RecentStreak::Ended { .. }) | None => 0,
    };
    StreakMarks {
        trail_length: calculate_trail_length(current_week_count),
        trail_strength: calculate_trail_strength(current_week_count),
        star_density_gain: calculate_star_density_gain(development.streaks.longest.len()),
    }
}

/// The comet trail's share of the arm for a running streak of `week_count` weeks:
/// linear to one at [`TRAIL_FULL_WEEKS`], then capped.
pub fn calculate_trail_length(week_count: usize) -> f64 {
    streak_fraction(week_count, TRAIL_FULL_WEEKS)
}

/// The comet trail's brightness for a running streak of `week_count` weeks: the
/// shortest running streak is already visible, then it rises linearly to one at
/// [`TRAIL_FULL_WEEKS`] and stays capped.
pub fn calculate_trail_strength(week_count: usize) -> f64 {
    match calculate_trail_length(week_count) {
        0.0 => 0.0,
        trail_length => 0.35 + 0.65 * trail_length,
    }
}

/// The longest-streak share that raises star density: linear to one at
/// [`STAR_DENSITY_FULL_WEEKS`], then capped.
pub fn calculate_star_density_gain(longest_week_count: usize) -> f64 {
    streak_fraction(longest_week_count, STAR_DENSITY_FULL_WEEKS)
}

/// The number of seeded stars the renderer draws for a longest streak of
/// `longest_week_count` weeks: [`BASE_STAR_COUNT`] plus its share of
/// [`EXTRA_STAR_COUNT`], rounded to the nearest star.
pub fn calculate_star_count(longest_week_count: usize) -> usize {
    let extra_stars = (calculate_star_density_gain(longest_week_count) * EXTRA_STAR_COUNT as f64)
        .round() as usize;
    BASE_STAR_COUNT + extra_stars
}

fn streak_fraction(week_count: usize, full_week_count: usize) -> f64 {
    (week_count as f64 / full_week_count as f64).min(1.0)
}
