//! A procedural blue galaxy drawn on the native 256 by 256 canvas.
//!
//! One pose comes from the account's identity seed: a tilted two-armed logarithmic
//! spiral, a glowing core, and seeded stars. Continuous brightness is quantized with an
//! ordered dither onto a fixed blue ramp, so a frame never exceeds one GIF palette. The
//! whole shape grows through five stages driven by total contributions, and the loop adds
//! a whole clockwise turn and periodic twinkle per 120 frames. This ticket renders the
//! stages and the blue palette; later tickets add colorways without changing the canvas
//! or encoder seam.

use std::f64::consts::{PI, TAU};

use crate::galaxy::{GalaxyDevelopment, STAGE_COUNT, calculate_stage};
use crate::motion;
use crate::theme::Theme;

/// Width of the native canvas every galaxy is drawn on.
pub const FRAME_WIDTH_PIXELS: u32 = 256;
/// Height of the native canvas; the canvas is square.
pub const FRAME_HEIGHT_PIXELS: u32 = FRAME_WIDTH_PIXELS;
/// Frames in one loop: a whole rotation and whole twinkle cycles.
pub const LOOP_FRAME_COUNT: u32 = 120;
/// The last stage's diameter as a share of the canvas width.
const FULL_STAGE_EXTENT_FRACTION: f64 = 0.85;
/// The first stage's outer radius: a single spark, far smaller than any arm or star field.
const SPARK_OUTER_RADIUS: f64 = 6.0;
/// The outer radius the shape constants below were authored against.
const BASE_OUTER_RADIUS: f64 = FRAME_WIDTH_PIXELS as f64 * 0.33;
/// How much of the outer radius the disc is visible across before fading out.
const FADE_START_FRACTION: f64 = 0.78;
/// How far seeded stars may reach, as a share of the stage's outer radius.
const STAR_OUTER_FRACTION: f64 = 0.98;

const PIXEL_COUNT: usize = (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize;
const CENTER_PIXELS: f64 = FRAME_WIDTH_PIXELS as f64 / 2.0;

/// Levels on the blue ramp, including the exact background at level zero.
const PALETTE_LEVELS: usize = 128;
/// How much the disc is squashed vertically, giving the spiral a face-on tilt.
const TILT: f64 = 0.78;
const ARM_PITCH: f64 = 2.3;
const CORE_RADIUS: f64 = 17.0;
const CORE_HALO_RADIUS: f64 = 38.0;
const DISK_FALLOFF_RADIUS: f64 = 42.0;
const ARM_FALLOFF_RADIUS: f64 = 52.0;
const STAR_COUNT: usize = 160;

/// A renderer-owned, fixed-size frame with row-major RGBA pixels.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GalaxyFrame {
    rgba_pixels: Vec<[u8; 4]>,
}

impl GalaxyFrame {
    /// The pixels as bytes, row by row from the top left, four channels per pixel: red,
    /// green, blue, alpha. The length is the canvas area times four.
    pub fn as_rgba_bytes(&self) -> &[u8] {
        self.rgba_pixels.as_flattened()
    }

    /// Build a frame from chosen pixels, for encoder tests that need colors no galaxy has.
    #[doc(hidden)]
    pub fn from_rgba_pixels(rgba_pixels: Vec<[u8; 4]>) -> Self {
        Self { rgba_pixels }
    }
}

/// Render one opaque still of the blue galaxy on the chosen theme background.
///
/// The identity seed fixes the spiral rotation, arm phase, texture, and stars; identical
/// developments and themes produce identical pixels without I/O, clock reads, or input
/// mutation. The still is loop frame zero, the base pose the loop turns from.
pub fn render_galaxy_still(development: &GalaxyDevelopment, theme: Theme) -> GalaxyFrame {
    render_galaxy_frame(development, theme, 0)
}

/// Render one frame of the seamless eight-second loop.
///
/// The base pose comes from the identity seed; the frame index adds a rotation of one
/// whole turn per loop and a periodic twinkle, both derived without incremental state.
/// Frame [`LOOP_FRAME_COUNT`] is frame zero, so the loop closes exactly. The total
/// contributions select the same growth stage for every still and loop frame. Identical
/// developments, themes, and frame indices produce identical pixels.
pub fn render_galaxy_frame(
    development: &GalaxyDevelopment,
    theme: Theme,
    frame_index: u32,
) -> GalaxyFrame {
    let palette = build_palette(theme);
    let pose = GalaxyPose::at_frame(development.identity_seed, frame_index);
    let stage_scale = stage_scale(calculate_stage(development.total_contributions));
    let mut brightness = vec![0.0; PIXEL_COUNT];
    paint_galaxy(&mut brightness, &pose, stage_scale);
    paint_stars(
        &mut brightness,
        &pose,
        development.identity_seed,
        frame_index,
        stage_scale,
    );
    let rgba_pixels = brightness
        .into_iter()
        .enumerate()
        .map(|(pixel_index, intensity)| {
            let threshold = ordered_threshold(
                pixel_index % FRAME_WIDTH_PIXELS as usize,
                pixel_index / FRAME_WIDTH_PIXELS as usize,
            );
            let position = intensity.clamp(0.0, 1.0) * (PALETTE_LEVELS - 1) as f64;
            let level = (position + threshold).floor() as usize;
            palette[level.min(PALETTE_LEVELS - 1)]
        })
        .collect();
    GalaxyFrame::from_rgba_pixels(rgba_pixels)
}

/// The fixed blue ramp per theme, from the exact background through dark blue to the
/// brightest core color. Ticket #5 replaces this with the six colorways.
fn blue_ramp(theme: Theme) -> [[u8; 3]; 7] {
    match theme {
        Theme::Dark => [
            [0x0d, 0x11, 0x17],
            [0x05, 0x12, 0x29],
            [0x1f, 0x39, 0x6a],
            [0x32, 0x59, 0x94],
            [0x4b, 0x82, 0xbf],
            [0x75, 0xb7, 0xe4],
            [0xba, 0xeb, 0xf9],
        ],
        Theme::Light => [
            [0xff, 0xff, 0xff],
            [0xc7, 0xef, 0xfa],
            [0x89, 0xc4, 0xe9],
            [0x5d, 0x95, 0xcc],
            [0x44, 0x73, 0xae],
            [0x32, 0x56, 0x8e],
            [0x23, 0x3a, 0x69],
        ],
    }
}

fn build_palette(theme: Theme) -> [[u8; 4]; PALETTE_LEVELS] {
    let anchors = blue_ramp(theme);
    let segment_count = anchors.len() - 1;
    let mut palette = [[0; 4]; PALETTE_LEVELS];
    for (level, color) in palette.iter_mut().enumerate() {
        let position = level as f64 / (PALETTE_LEVELS - 1) as f64 * segment_count as f64;
        let segment = (position.floor() as usize).min(segment_count - 1);
        let fraction = position - segment as f64;
        let from = anchors[segment];
        let to = anchors[segment + 1];
        *color = [
            interpolate_channel(from[0], to[0], fraction),
            interpolate_channel(from[1], to[1], fraction),
            interpolate_channel(from[2], to[2], fraction),
            0xff,
        ];
    }
    palette[0] = theme.background();
    palette
}

fn interpolate_channel(from: u8, to: u8, fraction: f64) -> u8 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * fraction).round() as u8
}

/// One seeded pose: a rigid rotation, an arm phase, and a texture salt.
struct GalaxyPose {
    rotation_cos: f64,
    rotation_sin: f64,
    arm_phase: f64,
    texture_seed: u64,
}

impl GalaxyPose {
    fn at_frame(seed: u64, frame_index: u32) -> Self {
        let mut rng = SeededRng::new(seed ^ 0x6761_6c61_7879);
        let rotation = rng.unit() * TAU + motion::frame_rotation(frame_index);
        let arm_phase = rng.unit() * TAU;
        Self {
            rotation_cos: rotation.cos(),
            rotation_sin: rotation.sin(),
            arm_phase,
            texture_seed: seed ^ 0x5354_4152,
        }
    }

    /// Screen offset from the canvas center to tilted disc coordinates.
    fn to_disc(&self, offset_x: f64, offset_y: f64) -> [f64; 2] {
        let rotated_x = offset_x * self.rotation_cos + offset_y * self.rotation_sin;
        let rotated_y = -offset_x * self.rotation_sin + offset_y * self.rotation_cos;
        [rotated_x, rotated_y / TILT]
    }

    /// Tilted disc coordinates to a screen offset from the canvas center.
    fn to_screen(&self, disc_x: f64, disc_y: f64) -> [f64; 2] {
        let rotated_x = disc_x;
        let rotated_y = disc_y * TILT;
        [
            rotated_x * self.rotation_cos - rotated_y * self.rotation_sin,
            rotated_x * self.rotation_sin + rotated_y * self.rotation_cos,
        ]
    }
}

/// Map a stage to the multiplier the shape's radii were authored against.
fn stage_scale(stage: u8) -> f64 {
    let step = f64::from(stage.clamp(1, STAGE_COUNT) - 1) / f64::from(STAGE_COUNT - 1);
    let full_radius = FRAME_WIDTH_PIXELS as f64 * FULL_STAGE_EXTENT_FRACTION / 2.0;
    let outer_radius = SPARK_OUTER_RADIUS + (full_radius - SPARK_OUTER_RADIUS) * step;
    outer_radius / BASE_OUTER_RADIUS
}

fn paint_galaxy(brightness: &mut [f64], pose: &GalaxyPose, scale: f64) {
    let outer_radius = BASE_OUTER_RADIUS * scale;
    let fade_start = outer_radius * FADE_START_FRACTION;
    for (pixel_index, value) in brightness.iter_mut().enumerate() {
        let offset_x = (pixel_index as u32 % FRAME_WIDTH_PIXELS) as f64 - CENTER_PIXELS;
        let offset_y = (pixel_index / FRAME_WIDTH_PIXELS as usize) as f64 - CENTER_PIXELS;
        let screen_radius = offset_x.hypot(offset_y);
        if screen_radius >= outer_radius {
            continue;
        }
        let [disc_x, disc_y] = pose.to_disc(offset_x, offset_y);
        let radius = disc_x.hypot(disc_y);
        let core = (-(radius / (CORE_RADIUS * scale)).powi(2)).exp()
            + 0.3 * (-(radius / (CORE_HALO_RADIUS * scale)).powi(2)).exp();
        let disk = 0.34 * (-radius / (DISK_FALLOFF_RADIUS * scale)).exp();
        let arm = 0.4 * arm_profile(disc_x, disc_y, radius, pose.arm_phase, scale);
        let texture = 0.9
            + 0.16 * value_noise(pose.texture_seed, disc_x * 0.11, disc_y * 0.11, 0)
            + 0.06 * value_noise(pose.texture_seed, disc_x * 0.31, disc_y * 0.31, 1);
        let fade = 1.0 - smooth_step(screen_radius, fade_start, outer_radius);
        *value = (core + (disk + arm) * texture) * fade;
    }
}

/// The two-armed logarithmic spiral's local brightness, gated away from the core and
/// falling off with radius.
fn arm_profile(disc_x: f64, disc_y: f64, radius: f64, arm_phase: f64, scale: f64) -> f64 {
    if radius < 1.0 {
        return 0.0;
    }
    let angle = disc_y.atan2(disc_x);
    let spiral_angle = (radius / (CORE_RADIUS * scale)).ln() * ARM_PITCH + arm_phase;
    let first_delta = wrap_angle(angle - spiral_angle);
    let second_delta = wrap_angle(first_delta - PI);
    let arm_width = 0.16 + 3.2 / radius;
    let first_arm = (-(first_delta / arm_width).powi(2)).exp();
    let second_arm = (-(second_delta / arm_width).powi(2)).exp();
    let gate = smooth_step(radius, 6.0 * scale, 24.0 * scale);
    gate * (-radius / (ARM_FALLOFF_RADIUS * scale)).exp() * first_arm.max(second_arm)
}

fn paint_stars(
    brightness: &mut [f64],
    pose: &GalaxyPose,
    identity_seed: u64,
    frame_index: u32,
    scale: f64,
) {
    let star_outer_radius = BASE_OUTER_RADIUS * STAR_OUTER_FRACTION * scale;
    let mut rng = SeededRng::new(pose.texture_seed);
    for star_index in 0..STAR_COUNT {
        let radius = star_outer_radius * rng.unit().powf(1.35);
        let angle = rng.unit() * TAU;
        let peak = (0.25 + 0.75 * rng.unit().powi(2))
            * motion::twinkle_scale(identity_seed, star_index, frame_index);
        let sigma = 0.55 + 0.55 * rng.unit();
        let [screen_x, screen_y] = pose.to_screen(radius * angle.cos(), radius * angle.sin());
        let center_x = CENTER_PIXELS + screen_x;
        let center_y = CENTER_PIXELS + screen_y;
        let reach = (sigma * 3.0).ceil() as i64;
        let left = (center_x - reach as f64).floor() as i64;
        let right = (center_x + reach as f64).ceil() as i64;
        let top = (center_y - reach as f64).floor() as i64;
        let bottom = (center_y + reach as f64).ceil() as i64;
        for pixel_y in top..=bottom {
            if pixel_y < 0 || pixel_y >= i64::from(FRAME_HEIGHT_PIXELS) {
                continue;
            }
            for pixel_x in left..=right {
                if pixel_x < 0 || pixel_x >= i64::from(FRAME_WIDTH_PIXELS) {
                    continue;
                }
                let distance_x = pixel_x as f64 - center_x;
                let distance_y = pixel_y as f64 - center_y;
                let falloff = (-(distance_x * distance_x + distance_y * distance_y)
                    / (2.0 * sigma * sigma))
                    .exp();
                brightness[pixel_y as usize * FRAME_WIDTH_PIXELS as usize + pixel_x as usize] +=
                    peak * falloff;
            }
        }
    }
}

fn wrap_angle(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Zero below `start`, one above `end`, smooth in between.
fn smooth_step(value: f64, start: f64, end: f64) -> f64 {
    let fraction = ((value - start) / (end - start)).clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// The 8 by 8 ordered-dither threshold at a pixel, from zero through one.
fn ordered_threshold(pixel_x: usize, pixel_y: usize) -> f64 {
    const BAYER: [[u8; 8]; 8] = [
        [0, 32, 8, 40, 2, 34, 10, 42],
        [48, 16, 56, 24, 50, 18, 58, 26],
        [12, 44, 4, 36, 14, 46, 6, 38],
        [60, 28, 52, 20, 62, 30, 54, 22],
        [3, 35, 11, 43, 1, 33, 9, 41],
        [51, 19, 59, 27, 49, 17, 57, 25],
        [15, 47, 7, 39, 13, 45, 5, 37],
        [63, 31, 55, 23, 61, 29, 53, 21],
    ];
    (f64::from(BAYER[pixel_y % 8][pixel_x % 8]) + 0.5) / 64.0
}

fn value_noise(seed: u64, coordinate_x: f64, coordinate_y: f64, salt: u64) -> f64 {
    let left = coordinate_x.floor();
    let top = coordinate_y.floor();
    let fraction_x = coordinate_x - left;
    let fraction_y = coordinate_y - top;
    let left = left as i64;
    let top = top as i64;
    let smooth_x = fraction_x * fraction_x * (3.0 - 2.0 * fraction_x);
    let smooth_y = fraction_y * fraction_y * (3.0 - 2.0 * fraction_y);
    let top_left = hash_unit(seed, left, top, salt);
    let top_right = hash_unit(seed, left + 1, top, salt);
    let bottom_left = hash_unit(seed, left, top + 1, salt);
    let bottom_right = hash_unit(seed, left + 1, top + 1, salt);
    let top_mix = top_left + (top_right - top_left) * smooth_x;
    let bottom_mix = bottom_left + (bottom_right - bottom_left) * smooth_x;
    top_mix + (bottom_mix - top_mix) * smooth_y
}

fn hash_unit(seed: u64, lattice_x: i64, lattice_y: i64, salt: u64) -> f64 {
    let mixed = splitmix64(
        seed ^ (lattice_x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (lattice_y as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
            ^ salt,
    );
    (mixed >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A small deterministic generator; no standard hasher and no randomness from the system.
struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self {
            state: splitmix64(seed),
        }
    }

    /// The next value in the half-open range from zero through one.
    fn unit(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mixed = splitmix64(self.state);
        (mixed >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

pub(crate) fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
