//! Image and animation encoding; callers own destinations and publication.
//!
//! Every export is [`STATIC_EXPORT_SCALE`] times the native canvas on each side, so the
//! pixel art stays crisp: [`encode_png`] for the still, [`encode_gif`] for a loop, and
//! [`encode_preview_html`] for a page that shows both themes' GIFs and PNGs on GitHub's
//! light and dark page colors.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame};

/// Each native pixel becomes this many export pixels on each side.
pub const STATIC_EXPORT_SCALE: u32 = 2;
/// Width of every exported image.
pub const STATIC_WIDTH_PIXELS: u32 = FRAME_WIDTH_PIXELS * STATIC_EXPORT_SCALE;
/// Height of every exported image.
pub const STATIC_HEIGHT_PIXELS: u32 = FRAME_HEIGHT_PIXELS * STATIC_EXPORT_SCALE;
/// GIF frames per second; one 120-frame loop lasts eight seconds.
pub const FRAMES_PER_SECOND: u32 = 15;

/// Encode a static PNG by repeating each native pixel in both dimensions.
/// Preserve its color and alpha without interpolation, timestamps, I/O, or mutation.
/// Encoding errors propagate before the caller publishes any resulting bytes.
pub fn encode_png(galaxy_frame: &GalaxyFrame) -> Result<Vec<u8>, png::EncodingError> {
    let native_pixels = galaxy_frame.as_rgba_bytes().as_chunks::<4>().0;
    let mut export_pixels = vec![[0; 4]; (STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS) as usize];
    for (pixel_index, pixel_channels) in export_pixels.iter_mut().enumerate() {
        let native_x = pixel_index as u32 % STATIC_WIDTH_PIXELS / STATIC_EXPORT_SCALE;
        let native_y = pixel_index as u32 / STATIC_WIDTH_PIXELS / STATIC_EXPORT_SCALE;
        *pixel_channels = native_pixels[(native_y * FRAME_WIDTH_PIXELS + native_x) as usize];
    }
    let mut png_bytes = Vec::new();
    let mut png_encoder =
        png::Encoder::new(&mut png_bytes, STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS);
    png_encoder.set_color(png::ColorType::Rgba);
    png_encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = png_encoder.write_header()?;
    png_writer.write_image_data(export_pixels.as_flattened())?;
    png_writer.finish()?;
    Ok(png_bytes)
}

/// The preview page, with `{caption}`, `{colorway}`, `{width}`, and `{height}` filled in.
const PREVIEW_TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>gitlaxy preview</title>
<style>
  body { margin: 0; font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; }
  h1, h2 { margin: 0; font-size: 14px; font-weight: 600; }
  section { display: grid; gap: 12px; padding: 16px; }
  .light { background: #ffffff; color: #1f2328; }
  .dark { background: #0d1117; color: #f0f6fc; }
  .images { display: flex; flex-wrap: wrap; gap: 16px; }
  figure { margin: 0; }
  img { display: block; max-width: 100%; height: auto; image-rendering: pixelated; }
</style>
</head>
<body>
<section class="light">
<h1>{caption}</h1>
<h2>Light</h2>
<div class="images">
<figure><img src="galaxy-light.gif" width="{width}" height="{height}" alt="The {colorway} galaxy on a light background"><figcaption>galaxy-light.gif</figcaption></figure>
<figure><img src="galaxy-light.png" width="{width}" height="{height}" alt="The {colorway} galaxy still on a light background"><figcaption>galaxy-light.png</figcaption></figure>
</div>
</section>
<section class="dark">
<h2>Dark</h2>
<div class="images">
<figure><img src="galaxy-dark.gif" width="{width}" height="{height}" alt="The {colorway} galaxy on a dark background"><figcaption>galaxy-dark.gif</figcaption></figure>
<figure><img src="galaxy-dark.png" width="{width}" height="{height}" alt="The {colorway} galaxy still on a dark background"><figcaption>galaxy-dark.png</figcaption></figure>
</div>
</section>
</body>
</html>
"#;

/// Build a page that previews `galaxy-light.gif` and `galaxy-light.png` on the light
/// background and `galaxy-dark.gif` and `galaxy-dark.png` on the dark background, read
/// from beside it, under the given caption and colorway name. It draws nothing itself and
/// loads no scripts or outside files, so it shows exactly what was generated.
pub fn encode_preview_html(caption: &str, colorway_name: &str) -> String {
    PREVIEW_TEMPLATE
        .replace("{colorway}", colorway_name)
        .replace("{width}", &STATIC_WIDTH_PIXELS.to_string())
        .replace("{height}", &STATIC_HEIGHT_PIXELS.to_string())
        // Last, so placeholder-like text in a caption stays literal.
        .replace("{caption}", &escape_html(caption))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// One GIF palette holds 256 entries, and one of them marks transparent pixels.
const GIF_COLOR_LIMIT: usize = 255;
const GIF_TRANSPARENT_INDEX: u8 = 0;

/// Encode a loop as an animated GIF at the static export size, repeating each native
/// pixel in both dimensions. The GIF repeats forever at [`FRAMES_PER_SECOND`]; each
/// delay, in hundredths of a second, is the step in a rounded running total, so a loop
/// lasts exactly its frame count divided by the frame rate. Each frame stores only the
/// box around its drawn pixels, with an exact palette of its colors, and that box clears
/// before the next frame. Colors are never approximated: a frame with more than 255
/// colors or a partly transparent pixel fails instead. Identical frames produce
/// identical bytes without I/O, timestamps, or input mutation.
pub fn encode_gif(galaxy_frames: &[GalaxyFrame]) -> Result<Vec<u8>, GifEncodingError> {
    if galaxy_frames.is_empty() {
        return Err(GifEncodingError::NoFrames);
    }
    let mut gif_encoder = gif::Encoder::new(
        Vec::new(),
        STATIC_WIDTH_PIXELS as u16,
        STATIC_HEIGHT_PIXELS as u16,
        &[],
    )?;
    gif_encoder.set_repeat(gif::Repeat::Infinite)?;
    for (frame_index, galaxy_frame) in galaxy_frames.iter().enumerate() {
        let native_pixels = galaxy_frame.as_rgba_bytes().as_chunks::<4>().0;
        let [box_left, box_top, box_width, box_height] = find_drawn_box(native_pixels);
        let mut palette_indices: BTreeMap<[u8; 3], u8> = BTreeMap::new();
        let mut palette_channels = vec![0; 3];
        let mut native_indices = Vec::with_capacity((box_width * box_height) as usize);
        for native_y in box_top..box_top + box_height {
            for native_x in box_left..box_left + box_width {
                let [red, green, blue, alpha] =
                    native_pixels[(native_y * FRAME_WIDTH_PIXELS + native_x) as usize];
                let palette_index = match alpha {
                    0 => GIF_TRANSPARENT_INDEX,
                    255 => match palette_indices.get(&[red, green, blue]) {
                        Some(&palette_index) => palette_index,
                        None if palette_indices.len() == GIF_COLOR_LIMIT => {
                            return Err(GifEncodingError::TooManyColors { frame_index });
                        }
                        None => {
                            let palette_index = palette_indices.len() as u8 + 1;
                            palette_indices.insert([red, green, blue], palette_index);
                            palette_channels.extend([red, green, blue]);
                            palette_index
                        }
                    },
                    _ => return Err(GifEncodingError::PartialTransparency { frame_index }),
                };
                native_indices.push(palette_index);
            }
        }
        let mut export_indices = Vec::with_capacity(
            native_indices.len() * (STATIC_EXPORT_SCALE * STATIC_EXPORT_SCALE) as usize,
        );
        for native_row in native_indices.chunks(box_width as usize) {
            let export_row: Vec<u8> = native_row
                .iter()
                .flat_map(|&palette_index| [palette_index; STATIC_EXPORT_SCALE as usize])
                .collect();
            for _ in 0..STATIC_EXPORT_SCALE {
                export_indices.extend_from_slice(&export_row);
            }
        }
        gif_encoder.write_frame(&gif::Frame {
            delay: calculate_elapsed_hundredths(frame_index + 1)
                - calculate_elapsed_hundredths(frame_index),
            dispose: gif::DisposalMethod::Background,
            transparent: Some(GIF_TRANSPARENT_INDEX),
            left: (box_left * STATIC_EXPORT_SCALE) as u16,
            top: (box_top * STATIC_EXPORT_SCALE) as u16,
            width: (box_width * STATIC_EXPORT_SCALE) as u16,
            height: (box_height * STATIC_EXPORT_SCALE) as u16,
            palette: Some(palette_channels),
            buffer: Cow::Owned(export_indices),
            ..gif::Frame::default()
        })?;
    }
    Ok(gif_encoder.into_inner()?)
}

/// The native box `[left, top, width, height]` around a frame's drawn pixels; a frame
/// with none keeps one transparent pixel, since a GIF frame cannot be empty.
fn find_drawn_box(native_pixels: &[[u8; 4]]) -> [u32; 4] {
    let mut drawn_edges: Option<[u32; 4]> = None;
    for (pixel_index, pixel_channels) in native_pixels.iter().enumerate() {
        if pixel_channels[3] == 0 {
            continue;
        }
        let pixel_x = pixel_index as u32 % FRAME_WIDTH_PIXELS;
        let pixel_y = pixel_index as u32 / FRAME_WIDTH_PIXELS;
        let [left, top, right, bottom] =
            drawn_edges.get_or_insert([pixel_x, pixel_y, pixel_x, pixel_y]);
        *left = (*left).min(pixel_x);
        *top = (*top).min(pixel_y);
        *right = (*right).max(pixel_x);
        *bottom = (*bottom).max(pixel_y);
    }
    drawn_edges.map_or([0, 0, 1, 1], |[left, top, right, bottom]| {
        [left, top, right + 1 - left, bottom + 1 - top]
    })
}

/// Hundredths of a second from the start of the loop to the given frame, rounded.
fn calculate_elapsed_hundredths(frame_index: usize) -> u16 {
    let frames_per_second = FRAMES_PER_SECOND as usize;
    u16::try_from((frame_index * 200 + frames_per_second) / (2 * frames_per_second))
        .expect("loops last under ten minutes")
}

/// Why a GIF was not produced.
#[derive(Debug)]
pub enum GifEncodingError {
    /// No frames were given.
    NoFrames,
    /// A frame has more opaque colors than one GIF palette holds.
    TooManyColors {
        /// The frame, counted from zero.
        frame_index: usize,
    },
    /// A frame has a pixel that is neither opaque nor fully transparent.
    PartialTransparency {
        /// The frame, counted from zero.
        frame_index: usize,
    },
    /// The GIF encoder failed; carries its own error.
    Encoder(gif::EncodingError),
}

impl fmt::Display for GifEncodingError {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => write!(output_formatter, "a GIF needs at least one frame"),
            Self::TooManyColors { frame_index } => write!(
                output_formatter,
                "frame {frame_index} has more than {GIF_COLOR_LIMIT} colors, more than one GIF palette holds"
            ),
            Self::PartialTransparency { frame_index } => write!(
                output_formatter,
                "frame {frame_index} has partly transparent pixels, which a GIF cannot show"
            ),
            Self::Encoder(encoding_error) => {
                write!(output_formatter, "GIF encoding failed: {encoding_error}")
            }
        }
    }
}

impl Error for GifEncodingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encoder(encoding_error) => Some(encoding_error),
            _ => None,
        }
    }
}

impl From<gif::EncodingError> for GifEncodingError {
    fn from(encoding_error: gif::EncodingError) -> Self {
        Self::Encoder(encoding_error)
    }
}
