//! The galaxy's per-frame pose inside the seamless eight-second loop.
//!
//! Rotation is a whole turn per loop and twinkle is a whole number of cycles per loop,
//! so both are exactly periodic: frame [`LOOP_FRAME_COUNT`] is frame zero. Nothing is
//! stored between frames; a frame's pose depends only on the identity seed and its index.

use std::f64::consts::TAU;

use crate::render::{LOOP_FRAME_COUNT, splitmix64};

/// Whole shimmer cycles per loop; the seam matches because the count is an integer.
const TWINKLE_CYCLES_PER_LOOP: f64 = 2.0;
/// A star may brighten or dim by at most this share of its peak.
const TWINKLE_DEPTH: f64 = 0.28;

/// The frame's place in the loop, wrapping the one-past-the-end frame onto zero.
pub fn loop_frame(frame_index: u32) -> u32 {
    frame_index % LOOP_FRAME_COUNT
}

/// The rotation added to the base pose, in radians: clockwise on screen, one whole turn
/// per loop. Positive angles are clockwise because the canvas's y axis points down.
pub fn frame_rotation(frame_index: u32) -> f64 {
    TAU * f64::from(loop_frame(frame_index)) / f64::from(LOOP_FRAME_COUNT)
}

/// One star's brightness multiplier at a frame, between `1 - TWINKLE_DEPTH` and
/// `1 + TWINKLE_DEPTH`. The star's position does not depend on it. The phase and depth
/// come from the identity seed and the star's index, so every star twinkles differently
/// but identically across runs.
pub fn twinkle_scale(identity_seed: u64, star_index: usize, frame_index: u32) -> f64 {
    let star_seed =
        splitmix64(identity_seed ^ (star_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let phase = ((star_seed >> 11) as f64 * (1.0 / (1u64 << 53) as f64)) * TAU;
    let depth = TWINKLE_DEPTH * (0.6 + 0.4 * ((star_seed >> 33) & 0xffff) as f64 / 65535.0);
    let angle = TWINKLE_CYCLES_PER_LOOP * frame_rotation(frame_index);
    1.0 + depth * (angle + phase).sin()
}
