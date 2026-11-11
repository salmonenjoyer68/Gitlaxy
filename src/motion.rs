//! The galaxy's per-frame pose inside the seamless eight-second loop.
//!
//! Rotation is a whole turn per loop and twinkle is a whole number of cycles per loop,
//! so both are exactly periodic: frame [`LOOP_FRAME_COUNT`] is frame zero. Nothing is
//! stored between frames; a frame's pose depends only on the identity seed, its index,
//! and the [`MotionState`]. Quiet is dimmer and twinkles sparsely and slowly; Active is
//! brighter, twinkles densely, and flares exactly twice per loop at seeded phases.

use std::f64::consts::TAU;

use crate::motion_state::MotionState;
use crate::render::{LOOP_FRAME_COUNT, splitmix64};

/// Whole shimmer cycles per loop; the seam matches because the count is an integer.
const ACTIVE_TWINKLE_CYCLES_PER_LOOP: f64 = 2.0;
const QUIET_TWINKLE_CYCLES_PER_LOOP: f64 = 1.0;
/// A star may brighten or dim by at most this share of its peak.
const ACTIVE_TWINKLE_DEPTH: f64 = 0.28;
const QUIET_TWINKLE_DEPTH: f64 = 0.10;
/// Share of stars that twinkle at all; quiet keeps most of them steady.
const QUIET_TWINKLE_SHARE: f64 = 0.3;
/// Overall brightness of a quiet galaxy relative to an active one.
const QUIET_BRIGHTNESS_SCALE: f64 = 0.7;
/// Frames either side of a flare's peak that carry part of its pulse.
const FLARE_HALF_WIDTH: u32 = 3;
/// Seeded offsets so the two flares never land close together or on one frame.
const FLARE_MIN_GAP: u32 = 45;
const FLARE_GAP_SPAN: u64 = 31;

/// The frame's place in the loop, wrapping the one-past-the-end frame onto zero.
pub fn loop_frame(frame_index: u32) -> u32 {
    frame_index % LOOP_FRAME_COUNT
}

/// The rotation added to the base pose, in radians: clockwise on screen, one whole turn
/// per loop. Positive angles are clockwise because the canvas's y axis points down.
pub fn frame_rotation(frame_index: u32) -> f64 {
    TAU * f64::from(loop_frame(frame_index)) / f64::from(LOOP_FRAME_COUNT)
}

/// One star's brightness multiplier at a frame. Quiet twinkles a third of the stars
/// slowly and shallowly; Active twinkles all of them through two cycles per loop.
/// The star's position does not depend on it. The phase and depth come from the
/// identity seed and the star's index, so every star twinkles differently but
/// identically across runs. A steady star returns exactly one.
pub fn twinkle_scale(
    identity_seed: u64,
    star_index: usize,
    frame_index: u32,
    state: MotionState,
) -> f64 {
    let star_seed =
        splitmix64(identity_seed ^ (star_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let twinkle_share = ((star_seed >> 50) & 0xff) as f64 / 255.0;
    if state == MotionState::Quiet && twinkle_share >= QUIET_TWINKLE_SHARE {
        return 1.0;
    }
    let phase = ((star_seed >> 11) as f64 * (1.0 / (1u64 << 53) as f64)) * TAU;
    let (cycles, max_depth) = match state {
        MotionState::Quiet => (QUIET_TWINKLE_CYCLES_PER_LOOP, QUIET_TWINKLE_DEPTH),
        MotionState::Active => (ACTIVE_TWINKLE_CYCLES_PER_LOOP, ACTIVE_TWINKLE_DEPTH),
    };
    let depth = max_depth * (0.6 + 0.4 * ((star_seed >> 33) & 0xffff) as f64 / 65535.0);
    let angle = cycles * frame_rotation(frame_index);
    1.0 + depth * (angle + phase).sin()
}

/// The two frames in the loop where Active flares peak, in the order they occur.
/// The identity seed fixes both phases, so a galaxy flares at the same moments every
/// run. Quiet never flares.
pub fn flare_frames(identity_seed: u64) -> [u32; 2] {
    let first = (splitmix64(identity_seed ^ 0x464c_4152_4553) % u64::from(LOOP_FRAME_COUNT)) as u32;
    let gap =
        FLARE_MIN_GAP + (splitmix64(identity_seed ^ 0x5048_4153_4553) % FLARE_GAP_SPAN) as u32;
    let second = (first + gap) % LOOP_FRAME_COUNT;
    [first.min(second), first.max(second)]
}

/// How strongly the galaxy flares at a frame, from zero through one. Each flare is a
/// short pulse with its peak exactly on its [`flare_frames`] entry; the pulse wraps
/// around the loop seam, so it stays periodic. Quiet has no flares.
pub fn flare_intensity(identity_seed: u64, frame_index: u32, state: MotionState) -> f64 {
    if state != MotionState::Active {
        return 0.0;
    }
    flare_frames(identity_seed)
        .into_iter()
        .map(|flare| {
            let distance = circular_frame_distance(loop_frame(frame_index), flare);
            if distance > FLARE_HALF_WIDTH {
                return 0.0;
            }
            let falloff = 1.0 - f64::from(distance) / f64::from(FLARE_HALF_WIDTH + 1);
            falloff * falloff
        })
        .fold(0.0, f64::max)
}

/// The brightness multiplier of the whole painted galaxy for a motion state: quiet
/// galaxies stay dim. Flares bloom on top in the renderer, not here.
pub fn brightness_scale(state: MotionState) -> f64 {
    match state {
        MotionState::Quiet => QUIET_BRIGHTNESS_SCALE,
        MotionState::Active => 1.0,
    }
}

fn circular_frame_distance(from: u32, to: u32) -> u32 {
    let forward = (to + LOOP_FRAME_COUNT - from) % LOOP_FRAME_COUNT;
    forward.min(LOOP_FRAME_COUNT - forward)
}
