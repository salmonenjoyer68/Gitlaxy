//! A procedural galaxy drawn on the native 256 by 256 canvas.
//!
//! One pose comes from the account's identity seed: a nearly face-on, broken three-armed
//! logarithmic pattern, a compact nucleus inside a faint bulge, and seeded stars.
//! Continuous brightness is quantized with an ordered dither onto a fixed 128-level ramp
//! built from the chosen [`Colorway`]'s palette, so a frame never exceeds one GIF
//! palette. The whole shape grows
//! through five stages driven by total contributions. The loop adds a whole clockwise
//! turn and a periodic twinkle per 120 frames; a [`MotionState`] dims quiet accounts and
//! adds two seeded flares to active ones. A running streak adds a comet trail along one
//! arm, the longest streak raises the star count, and language pigments tint a subset of
//! stars and arm accents.

use std::f64::consts::{PI, TAU};

use crate::colorway::{Colorway, Palette};
use crate::galaxy::{GalaxyDevelopment, STAGE_COUNT, calculate_stage};
use crate::markings::{StreakMarks, calculate_star_count, calculate_streak_marks};
use crate::motion;
use crate::motion_state;
use crate::motion_state::MotionState;
use crate::pigment::{Pigment, PigmentShare, pigment_color, pigment_shares, select_pigment};
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
/// How far seeded stars may reach, as a share of the stage's outer radius.
const STAR_OUTER_FRACTION: f64 = 0.98;

const PIXEL_COUNT: usize = (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize;
const CENTER_PIXELS: f64 = FRAME_WIDTH_PIXELS as f64 / 2.0;

/// Levels on the dithered ramp, including the exact background at level zero.
const PALETTE_LEVELS: usize = 128;
/// How much the disc is squashed vertically: nearly face-on.
const TILT: f64 = 0.96;
/// Primary arms in the broken spiral. The count is an artistic approximation, not a
/// measured physical component count.
const PRIMARY_ARM_COUNT: usize = 3;
/// How far the arm guide winds from the core radius outward.
const ARM_PITCH: f64 = 1.4;
/// Radius the arm guide's logarithm is measured against, at the base stage scale.
const CORE_RADIUS: f64 = 17.0;
/// Peak added brightness of a primary arm's ridge.
const ARM_PEAK: f64 = 1.6;
/// Radial e-folding length of the arm light at the base stage scale.
const ARM_FALLOFF_RADIUS: f64 = 52.0;
/// A compact, intense nucleus inside a much fainter, wider bulge.
const NUCLEUS_RADIUS: f64 = 5.0;
const NUCLEUS_PEAK: f64 = 2.0;
const BULGE_RADIUS: f64 = 30.0;
const BULGE_PEAK: f64 = 0.2;
/// Peak brightness of the faint extended disc, and its e-folding share of the outer
/// radius; low enough that the interarm regions stay dark.
const DISK_PEAK: f64 = 0.13;
const DISK_FALLOFF_SHARE: f64 = 0.62;
/// Peak added brightness of a flare blooming out of the galaxy's core.
const FLARE_GLOW_PEAK: f64 = 0.75;
/// Radius the flare's glow fades across, in canvas pixels.
const FLARE_GLOW_RADIUS: f64 = 28.0;
/// Peak brightness the comet trail adds at full strength.
const TRAIL_PEAK_BRIGHTNESS: f64 = 0.55;
/// Half-width of the comet trail across the arm, in pixels.
const TRAIL_WIDTH_PIXELS: f64 = 1.8;
/// Peak brightness a dust mote adds mid-fall.
const MOTE_PEAK_BRIGHTNESS: f64 = 0.8;
/// Mote glow half-width range, in canvas pixels.
const MOTE_SIGMA_MIN: f64 = 0.45;
const MOTE_SIGMA_SPAN: f64 = 0.45;
/// Seed salt keeps the mote field independent of the stars, texture, and pigments.
const MOTE_SALT: u64 = 0x4d4f_5445;
/// Seed salts keep the arm shapes, their broken light, and the rim separate: changing one
/// field does not reshuffle the rest.
const ARM_SHAPE_SALT: u64 = 0x4152_4d53_4841_5045;
const ARM_TEXTURE_SALT: u64 = 0x4152_4d54_4558_5452;
const RIM_SALT: u64 = 0x5249_4d46_4144_4553;

/// Share of stars whose core takes a pigment color; the rest stay in the colorway.
const PIGMENT_STAR_FRACTION: f64 = 0.42;
/// A star core carries its pigment only inside this share of its peak brightness.
const PIGMENT_STAR_CORE_FALLOFF: f64 = 0.32;
/// Share of disc pixels allowed to take an accent color; the colorway keeps the rest.
const PIGMENT_PIXEL_SHARE: f64 = 0.3;
/// Spatial scale of organic arm-accent patches, in disc pixels per noise cell.
const PIGMENT_ACCENT_SCALE: f64 = 0.09;
/// Arm brightness below which an accent patch is too faint to tint.
const PIGMENT_ACCENT_ARM: f64 = 0.07;
/// Noise above which an arm pixel joins a pigment accent patch.
const PIGMENT_ACCENT_THRESHOLD: f64 = 0.88;
/// Seed salts keep pigment decisions independent of the texture and the star field.
const PIGMENT_ARM_SALT: u64 = 0x5049_474d_454e_5431;
const PIGMENT_SELECT_SALT: u64 = 0x5049_474d_454e_5432;
const PIGMENT_STAR_SALT: u64 = 0x5049_474d_454e_5433;
const PIGMENT_PIXEL_SALT: u64 = 0x5049_474d_454e_5434;

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

/// Render one opaque still of the galaxy in `colorway` on the chosen theme background.
///
/// The identity seed fixes the spiral rotation, arm phase, texture, and stars; identical
/// developments, colorways, and themes produce identical pixels without I/O, clock
/// reads, or input mutation. The still is loop frame zero, the base pose the loop turns
/// from.
pub fn render_galaxy_still(
    development: &GalaxyDevelopment,
    colorway: Colorway,
    theme: Theme,
) -> GalaxyFrame {
    render_galaxy_frame(development, colorway, theme, 0)
}

/// Render one frame of the seamless eight-second loop.
///
/// The base pose comes from the identity seed; the frame index adds a rotation of one
/// whole turn per loop and a periodic twinkle, both derived without incremental state.
/// Frame [`LOOP_FRAME_COUNT`] is frame zero, so the loop closes exactly. The total
/// contributions select the same growth stage for every still and loop frame. Identical
/// developments, colorways, themes, and frame indices produce identical pixels.
pub fn render_galaxy_frame(
    development: &GalaxyDevelopment,
    colorway: Colorway,
    theme: Theme,
    frame_index: u32,
) -> GalaxyFrame {
    let palette = build_palette(colorway.palette(theme));
    let pose = GalaxyPose::at_frame(development.identity_seed, frame_index);
    let motion_state = motion_state::select_motion_state(development);
    let brightness_scale = motion::brightness_scale(motion_state);
    let flare_intensity =
        motion::flare_intensity(development.identity_seed, frame_index, motion_state);
    let stage_scale = stage_scale(calculate_stage(development.total_contributions));
    let language_pigments = pigment_shares(development);
    let streak_marks = calculate_streak_marks(development);
    let mut tint = vec![None; PIXEL_COUNT];
    let mut brightness = vec![0.0; PIXEL_COUNT];
    paint_galaxy(
        &mut brightness,
        &mut tint,
        &pose,
        stage_scale,
        flare_intensity,
        &language_pigments,
    );
    paint_trail(&mut brightness, &pose, stage_scale, &streak_marks);
    paint_stars(
        &mut brightness,
        &mut tint,
        &StarfieldPaint {
            pose: &pose,
            identity_seed: development.identity_seed,
            frame_index,
            state: motion_state,
            scale: stage_scale,
            pigment_shares: &language_pigments,
            star_count: calculate_star_count(development.streaks.longest.len()),
        },
    );
    paint_motes(
        &mut brightness,
        &pose,
        stage_scale,
        frame_index,
        motion_state,
    );
    let rgba_pixels = brightness
        .into_iter()
        .enumerate()
        .map(|(pixel_index, intensity)| {
            let threshold = ordered_threshold(
                pixel_index % FRAME_WIDTH_PIXELS as usize,
                pixel_index / FRAME_WIDTH_PIXELS as usize,
            );
            let position =
                (intensity * brightness_scale).clamp(0.0, 1.0) * (PALETTE_LEVELS - 1) as f64;
            let level = (position + threshold).floor() as usize;
            let level = level.min(PALETTE_LEVELS - 1);
            match tint[pixel_index] {
                Some(pigment) if level > 0 => {
                    pigment_color(pigment, theme, level as f64 / (PALETTE_LEVELS - 1) as f64)
                }
                _ => palette[level],
            }
        })
        .collect();
    GalaxyFrame::from_rgba_pixels(rgba_pixels)
}

/// Interpolate the colorway's anchors into the dithered ramp, background first.
fn build_palette(palette: Palette) -> [[u8; 4]; PALETTE_LEVELS] {
    let anchors = palette.anchors;
    let segment_count = anchors.len() - 1;
    let mut ramp = [[0; 4]; PALETTE_LEVELS];
    for (level, color) in ramp.iter_mut().enumerate() {
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
    let background = anchors[0];
    ramp[0] = [background[0], background[1], background[2], 0xff];
    ramp
}

fn interpolate_channel(from: u8, to: u8, fraction: f64) -> u8 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * fraction).round() as u8
}

/// One primary arm's seeded shape, as dimensionless factors applied at a stage's scale.
#[derive(Clone, Copy)]
struct ArmShape {
    /// Multiplier on the arm's ridge brightness.
    strength: f64,
    /// Multiplier on the ridge's angular width.
    width: f64,
    /// Multiplier on the arm's radial e-folding length.
    reach: f64,
    /// Peak brightness of the arm's short outer branch; zero leaves the arm unbranched.
    branch_strength: f64,
    /// The branch's angular offset from the arm's centerline, in radians.
    branch_offset: f64,
}

/// The three primary arms' seeded shapes. Two arms carry a faint short branch and one is
/// left bare, so the fan stays visibly asymmetric.
fn seeded_arm_shapes(seed: u64) -> [ArmShape; PRIMARY_ARM_COUNT] {
    let bare_arm = (hash_unit(seed, 0, 4, ARM_SHAPE_SALT) * PRIMARY_ARM_COUNT as f64) as usize
        % PRIMARY_ARM_COUNT;
    let branch_offset = 0.5 + 0.55 * hash_unit(seed, 0, 5, ARM_SHAPE_SALT);
    let mut shapes = [ArmShape {
        strength: 1.0,
        width: 1.0,
        reach: 1.0,
        branch_strength: 0.0,
        branch_offset,
    }; PRIMARY_ARM_COUNT];
    for (arm_index, shape) in shapes.iter_mut().enumerate() {
        let arm_row = arm_index as i64;
        let branched = arm_index != bare_arm;
        *shape = ArmShape {
            strength: 0.72 + 0.5 * hash_unit(seed, arm_row, 0, ARM_SHAPE_SALT),
            width: 0.7 + 0.65 * hash_unit(seed, arm_row, 1, ARM_SHAPE_SALT),
            reach: 0.78 + 0.5 * hash_unit(seed, arm_row, 2, ARM_SHAPE_SALT),
            branch_strength: if branched {
                0.24 + 0.16 * hash_unit(seed, arm_row, 3, ARM_SHAPE_SALT)
            } else {
                0.0
            },
            branch_offset,
        };
    }
    shapes
}

/// One seeded pose: a rigid rotation, the pattern's arm phase, the arms' seeded shapes,
/// the arm the comet trail follows, and a texture salt.
struct GalaxyPose {
    rotation_cos: f64,
    rotation_sin: f64,
    arm_phase: f64,
    arm_shapes: [ArmShape; PRIMARY_ARM_COUNT],
    primary_arm: usize,
    texture_seed: u64,
}

impl GalaxyPose {
    fn at_frame(seed: u64, frame_index: u32) -> Self {
        let mut rng = SeededRng::new(seed ^ 0x6761_6c61_7879);
        let rotation = rng.unit() * TAU + motion::frame_rotation(frame_index);
        let arm_phase = rng.unit() * TAU;
        let arm_shapes = seeded_arm_shapes(seed);
        let primary_arm = arm_shapes
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| {
                (left.strength * left.reach).total_cmp(&(right.strength * right.reach))
            })
            .map(|(arm_index, _)| arm_index)
            .unwrap_or(0);
        Self {
            rotation_cos: rotation.cos(),
            rotation_sin: rotation.sin(),
            arm_phase,
            arm_shapes,
            primary_arm,
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

/// The radius of the star field's rim at a stage scale.
fn starfield_outer_radius(scale: f64) -> f64 {
    BASE_OUTER_RADIUS * STAR_OUTER_FRACTION * scale
}

fn paint_galaxy(
    brightness: &mut [f64],
    tint: &mut [Option<Pigment>],
    pose: &GalaxyPose,
    scale: f64,
    flare_intensity: f64,
    pigment_shares: &[PigmentShare],
) {
    let outer_radius = BASE_OUTER_RADIUS * scale;
    let disc_falloff = outer_radius * DISK_FALLOFF_SHARE;
    for (pixel_index, value) in brightness.iter_mut().enumerate() {
        let offset_x = (pixel_index as u32 % FRAME_WIDTH_PIXELS) as f64 - CENTER_PIXELS;
        let offset_y = (pixel_index / FRAME_WIDTH_PIXELS as usize) as f64 - CENTER_PIXELS;
        let screen_radius = offset_x.hypot(offset_y);
        if screen_radius >= outer_radius {
            continue;
        }
        let [disc_x, disc_y] = pose.to_disc(offset_x, offset_y);
        let radius = disc_x.hypot(disc_y);
        let angle = disc_y.atan2(disc_x);
        let nucleus = NUCLEUS_PEAK * (-(radius / (NUCLEUS_RADIUS * scale)).powi(2)).exp();
        let bulge = BULGE_PEAK * (-(radius / (BULGE_RADIUS * scale)).powi(2)).exp();
        let disk = DISK_PEAK * (-radius / disc_falloff).exp();
        let arm = ARM_PEAK
            * arm_profile(
                radius,
                angle,
                pose.arm_phase,
                scale,
                &pose.arm_shapes,
                pose.texture_seed,
            );
        let texture = 0.92 + 0.10 * value_noise(pose.texture_seed, disc_x * 0.09, disc_y * 0.09, 0);
        let fade_start = rim_fade_start(pose.texture_seed, offset_y.atan2(offset_x), outer_radius);
        let fade = 1.0 - smooth_step(screen_radius, fade_start, outer_radius);
        let flare = flare_intensity
            * FLARE_GLOW_PEAK
            * (-(screen_radius / (FLARE_GLOW_RADIUS * 0.67)).powi(2)).exp()
            * (1.0 - smooth_step(screen_radius, FLARE_GLOW_RADIUS * 0.83, FLARE_GLOW_RADIUS));
        *value = (nucleus + bulge + disk + arm * texture) * fade + flare;
        if !pigment_shares.is_empty() {
            let accent_coordinate_x = disc_x * PIGMENT_ACCENT_SCALE;
            let accent_coordinate_y = disc_y * PIGMENT_ACCENT_SCALE;
            let accent_patch = value_noise(
                pose.texture_seed,
                accent_coordinate_x,
                accent_coordinate_y,
                PIGMENT_ARM_SALT,
            );
            if arm > PIGMENT_ACCENT_ARM
                && accent_patch > PIGMENT_ACCENT_THRESHOLD
                && is_pigment_accent_pixel(pose, disc_x, disc_y)
            {
                let accent_unit = hash_unit(
                    pose.texture_seed,
                    accent_coordinate_x.floor() as i64,
                    accent_coordinate_y.floor() as i64,
                    PIGMENT_SELECT_SALT,
                );
                tint[pixel_index] = select_pigment(pigment_shares, accent_unit);
            }
        }
    }
}

/// The centerline angle of one primary arm at a disc radius. The outer angle lags the
/// pattern phase, so each arm trails clockwise; a non-positive radius has no logarithm.
fn arm_centerline(radius: f64, arm_index: usize, arm_phase: f64, scale: f64) -> f64 {
    let core_radius = CORE_RADIUS * scale;
    let wind = if radius > core_radius {
        (radius / core_radius).ln()
    } else {
        0.0
    };
    arm_phase - wind * ARM_PITCH + arm_index as f64 * TAU / PRIMARY_ARM_COUNT as f64
}

/// The broken three-armed pattern's local brightness: a gaussian ridge along each arm's
/// shared centerline, split into uneven luminous sections, plus a faint shorter branch on
/// two of the arms.
fn arm_profile(
    radius: f64,
    angle: f64,
    arm_phase: f64,
    scale: f64,
    shapes: &[ArmShape; PRIMARY_ARM_COUNT],
    texture_seed: u64,
) -> f64 {
    let gate = smooth_step(radius, 5.0 * scale, 22.0 * scale);
    if gate <= 0.0 {
        return 0.0;
    }
    let mut brightness = 0.0;
    for (arm_index, shape) in shapes.iter().enumerate() {
        let arm_length = ARM_FALLOFF_RADIUS * scale * shape.reach;
        let falloff = (-radius / arm_length).exp();
        if falloff < 0.01 {
            continue;
        }
        let arm_row = arm_index as f64 * 7.0;
        let centerline = arm_centerline(radius, arm_index, arm_phase, scale);
        let width = shape.width * (0.09 + 2.2 / radius);
        let ridge = (-(wrap_angle(angle - centerline) / width).powi(2)).exp();
        let sections = 0.35
            + 0.65
                * smooth_step(
                    value_noise(texture_seed, radius * 0.075, arm_row, ARM_TEXTURE_SALT),
                    0.28,
                    0.68,
                );
        let knots =
            0.85 + 0.3 * value_noise(texture_seed, radius * 0.11, arm_row, ARM_TEXTURE_SALT ^ 1);
        brightness += shape.strength * ridge * falloff * sections * knots;
        if shape.branch_strength <= 0.0 {
            continue;
        }
        let branch_start = arm_length * 0.42;
        let branch_end = arm_length * 0.98;
        let branch_band = smooth_step(radius, branch_start, branch_start + 9.0 * scale)
            * (1.0 - smooth_step(radius, branch_end - 12.0 * scale, branch_end));
        if branch_band <= 0.0 {
            continue;
        }
        let branch_angle = centerline + shape.branch_offset;
        let branch_width = width * 0.85;
        let branch_ridge = (-(wrap_angle(angle - branch_angle) / branch_width).powi(2)).exp();
        brightness += shape.branch_strength * branch_ridge * falloff * branch_band;
    }
    gate * brightness
}

/// Where the disc starts fading at a screen angle. Two noise octaves make the rim
/// irregular, so the outer light never ends on a hard circle. The fade is fixed in the
/// frame rather than turning with the pattern: it is a vignette on the canvas, and a
/// rotating rim would change most of the faint outer pixels every frame.
fn rim_fade_start(texture_seed: u64, angle: f64, outer_radius: f64) -> f64 {
    let (cos_angle, sin_angle) = (angle.cos(), angle.sin());
    let broad = value_noise(texture_seed, 2.4 * cos_angle, 2.4 * sin_angle, RIM_SALT);
    let fine = value_noise(texture_seed, 6.0 * cos_angle, 6.0 * sin_angle, RIM_SALT ^ 1);
    outer_radius * (0.66 + 0.18 * (0.7 * broad + 0.3 * fine))
}

/// Add the running streak's comet glint along the strongest primary arm.
///
/// The arm is chosen from the seeded pose, so the trail follows luminous material rather
/// than an interarm gap. The trail starts just outside the core and covers
/// `trail_length` of the arm, so a longer streak reaches further out; its brightness is
/// `trail_strength`. It is drawn in disc coordinates, so it turns with the galaxy and the
/// loop closes exactly. With no running streak the trail length is zero and nothing is
/// painted.
fn paint_trail(brightness: &mut [f64], pose: &GalaxyPose, scale: f64, streak_marks: &StreakMarks) {
    if streak_marks.trail_length <= 0.0 {
        return;
    }
    let core_radius = CORE_RADIUS * scale;
    let outer_radius = starfield_outer_radius(scale);
    let start_radius = core_radius * 1.5;
    let end_radius = start_radius + (outer_radius - start_radius) * streak_marks.trail_length;
    let taper_width = 2.0 * scale;
    for (pixel_index, value) in brightness.iter_mut().enumerate() {
        let offset_x = (pixel_index as u32 % FRAME_WIDTH_PIXELS) as f64 - CENTER_PIXELS;
        let offset_y = (pixel_index / FRAME_WIDTH_PIXELS as usize) as f64 - CENTER_PIXELS;
        let [disc_x, disc_y] = pose.to_disc(offset_x, offset_y);
        let radius = disc_x.hypot(disc_y);
        if radius < start_radius || radius >= end_radius {
            continue;
        }
        let centerline = arm_centerline(radius, pose.primary_arm, pose.arm_phase, scale);
        let lateral_distance = wrap_angle(disc_y.atan2(disc_x) - centerline) * radius;
        let across = (-(lateral_distance / TRAIL_WIDTH_PIXELS).powi(2)).exp();
        let along = smooth_step(radius, start_radius, start_radius + taper_width);
        *value += TRAIL_PEAK_BRIGHTNESS * streak_marks.trail_strength * across * along;
    }
}

/// The seeded inputs one star field is painted from.
#[derive(Clone, Copy)]
struct StarfieldPaint<'a> {
    pose: &'a GalaxyPose,
    identity_seed: u64,
    frame_index: u32,
    state: MotionState,
    scale: f64,
    pigment_shares: &'a [PigmentShare],
    star_count: usize,
}

/// Add one Gaussian splat's light to the frame, clipped to the canvas.
fn splat_gaussian(brightness: &mut [f64], center_x: f64, center_y: f64, sigma: f64, peak: f64) {
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
            let pixel_index = pixel_y as usize * FRAME_WIDTH_PIXELS as usize + pixel_x as usize;
            brightness[pixel_index] += peak * falloff;
        }
    }
}

fn paint_stars(
    brightness: &mut [f64],
    tint: &mut [Option<Pigment>],
    starfield: &StarfieldPaint<'_>,
) {
    let StarfieldPaint {
        pose,
        identity_seed,
        frame_index,
        state,
        scale,
        pigment_shares,
        star_count,
    } = *starfield;
    let star_outer_radius = starfield_outer_radius(scale);
    let mut rng = SeededRng::new(pose.texture_seed);
    for star_index in 0..star_count {
        let radius = star_outer_radius * rng.unit().powf(1.35);
        let angle = rng.unit() * TAU;
        let peak = (0.25 + 0.75 * rng.unit().powi(2))
            * motion::twinkle_scale(identity_seed, star_index, frame_index, state);
        let sigma = 0.55 + 0.55 * rng.unit();
        let star_pigment = if !pigment_shares.is_empty()
            && hash_unit(identity_seed, star_index as i64, 0, PIGMENT_STAR_SALT)
                < PIGMENT_STAR_FRACTION
        {
            select_pigment(
                pigment_shares,
                hash_unit(identity_seed, star_index as i64, 1, PIGMENT_SELECT_SALT),
            )
        } else {
            None
        };
        let [screen_x, screen_y] = pose.to_screen(radius * angle.cos(), radius * angle.sin());
        let center_x = CENTER_PIXELS + screen_x;
        let center_y = CENTER_PIXELS + screen_y;
        splat_gaussian(brightness, center_x, center_y, sigma, peak);
        let Some(pigment) = star_pigment else {
            continue;
        };
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
                if falloff > PIGMENT_STAR_CORE_FALLOFF {
                    let [disc_x, disc_y] = pose.to_disc(
                        pixel_x as f64 - CENTER_PIXELS,
                        pixel_y as f64 - CENTER_PIXELS,
                    );
                    if is_pigment_accent_pixel(pose, disc_x, disc_y) {
                        let pixel_index =
                            pixel_y as usize * FRAME_WIDTH_PIXELS as usize + pixel_x as usize;
                        tint[pixel_index] = Some(pigment);
                    }
                }
            }
        }
    }
}

/// Paint dust motes falling from the rim into the core.
fn paint_motes(
    brightness: &mut [f64],
    pose: &GalaxyPose,
    scale: f64,
    frame_index: u32,
    state: MotionState,
) {
    let outer_radius = starfield_outer_radius(scale);
    let mote_count = (motion::mote_count(state) as f64 * scale).ceil().max(1.0) as usize;
    let mut rng = SeededRng::new(pose.texture_seed ^ MOTE_SALT);
    for _ in 0..mote_count {
        let phase = motion::mote_phase(frame_index, rng.unit());
        let angle = rng.unit() * TAU;
        let sigma = MOTE_SIGMA_MIN + MOTE_SIGMA_SPAN * rng.unit();
        let peak = MOTE_PEAK_BRIGHTNESS * (0.6 + 0.4 * rng.unit()) * motion::mote_brightness(phase);
        if peak <= 0.0 {
            continue;
        }
        let radius = outer_radius * motion::mote_radius_share(phase);
        let [screen_x, screen_y] = pose.to_screen(radius * angle.cos(), radius * angle.sin());
        let center_x = CENTER_PIXELS + screen_x;
        let center_y = CENTER_PIXELS + screen_y;
        splat_gaussian(brightness, center_x, center_y, sigma, peak);
    }
}

/// Whether one disc pixel may take a pigment accent; the rest keep the colorway.
fn is_pigment_accent_pixel(pose: &GalaxyPose, disc_x: f64, disc_y: f64) -> bool {
    hash_unit(
        pose.texture_seed,
        disc_x.floor() as i64,
        disc_y.floor() as i64,
        PIGMENT_PIXEL_SALT,
    ) < PIGMENT_PIXEL_SHARE
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
