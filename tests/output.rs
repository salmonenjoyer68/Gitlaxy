use std::io::Cursor;
use std::sync::LazyLock;

use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::derive_galaxy_development;
use gitlaxy::history::AccountSnapshot;
use gitlaxy::output::{
    FRAMES_PER_SECOND, GifEncodingError, STATIC_HEIGHT_PIXELS, STATIC_WIDTH_PIXELS, encode_gif,
    encode_png, encode_preview_html,
};
use gitlaxy::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame, render_galaxy_still};
use gitlaxy::theme::Theme;

const FRAME_PIXEL_COUNT: usize = (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize;

#[test]
fn the_gif_rebuilds_every_frame_exactly_at_twice_the_size() {
    let galaxy_frames = two_theme_frames();
    let gif_bytes = encode_gif(&galaxy_frames).unwrap();
    assert_eq!(encode_gif(&galaxy_frames).unwrap(), gif_bytes);
    let mut gif_options = gif::DecodeOptions::new();
    gif_options.set_color_output(gif::ColorOutput::Indexed);
    let mut gif_decoder = gif_options.read_info(gif_bytes.as_slice()).unwrap();
    assert_eq!(
        (
            u32::from(gif_decoder.width()),
            u32::from(gif_decoder.height())
        ),
        (STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS)
    );
    // Composite the way a browser does: draw each frame's box over the canvas, show
    // the result, then apply the frame's disposal before the next one.
    let mut canvas = vec![[0; 4]; (STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS) as usize];
    for galaxy_frame in galaxy_frames {
        let gif_frame = gif_decoder.read_next_frame().unwrap().unwrap();
        assert_eq!(
            (u32::from(gif_frame.width), u32::from(gif_frame.height)),
            (STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS)
        );
        let palette = gif_frame.palette.as_ref().unwrap();
        let box_pixels = |box_index: usize| {
            let canvas_x = usize::from(gif_frame.left) + box_index % usize::from(gif_frame.width);
            let canvas_y = usize::from(gif_frame.top) + box_index / usize::from(gif_frame.width);
            canvas_y * STATIC_WIDTH_PIXELS as usize + canvas_x
        };
        for (box_index, &palette_index) in gif_frame.buffer.iter().enumerate() {
            if Some(palette_index) != gif_frame.transparent {
                let color_start = usize::from(palette_index) * 3;
                let [red, green, blue] = palette[color_start..color_start + 3] else {
                    unreachable!()
                };
                canvas[box_pixels(box_index)] = [red, green, blue, 255];
            }
        }
        assert_eq!(canvas, scale_to_export(&galaxy_frame));
        if gif_frame.dispose == gif::DisposalMethod::Background {
            for box_index in 0..gif_frame.buffer.len() {
                canvas[box_pixels(box_index)] = [0; 4];
            }
        }
    }
    assert!(gif_decoder.read_next_frame().unwrap().is_none());
}

#[test]
fn the_png_repeats_each_native_pixel_exactly_at_twice_the_size() {
    let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, Theme::Dark);
    let png_bytes = encode_png(&galaxy_frame).unwrap();
    let mut png_reader = png::Decoder::new(Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    let mut png_buffer = vec![0; png_reader.output_buffer_size().unwrap()];
    let png_info = png_reader.next_frame(&mut png_buffer).unwrap();
    assert_eq!(
        (png_info.width, png_info.height),
        (STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS)
    );
    assert_eq!(png_info.color_type, png::ColorType::Rgba);
    png_buffer.truncate(png_info.buffer_size());
    let export_pixels: Vec<[u8; 4]> = png_buffer.as_chunks::<4>().0.to_vec();
    assert_eq!(export_pixels, scale_to_export(&galaxy_frame));
}

#[test]
fn the_gif_repeats_forever_at_fifteen_frames_per_second() {
    let (_, gif_bytes) = &*ENCODED_STILL_LOOP;
    let mut gif_decoder = gif::DecodeOptions::new()
        .read_info(gif_bytes.as_slice())
        .unwrap();
    let mut frame_count = 0;
    let mut total_hundredths = 0;
    while let Some(gif_frame) = gif_decoder.read_next_frame().unwrap() {
        frame_count += 1;
        total_hundredths += u32::from(gif_frame.delay);
        assert!(gif_frame.delay == 6 || gif_frame.delay == 7);
    }
    assert_eq!(frame_count, 120);
    assert_eq!(total_hundredths, 800);
    assert_eq!(FRAMES_PER_SECOND, 15);
    assert_eq!(gif_decoder.repeat(), gif::Repeat::Infinite);
}

#[test]
fn an_empty_loop_is_an_error() {
    assert!(matches!(encode_gif(&[]), Err(GifEncodingError::NoFrames)));
}

#[test]
fn a_frame_may_use_every_palette_color_but_no_more() {
    assert!(encode_gif(&[frame_with_colors(distinct_colors(255))]).is_ok());
    let crowded_frames = [
        frame_with_colors(distinct_colors(1)),
        frame_with_colors(distinct_colors(256)),
    ];
    assert!(matches!(
        encode_gif(&crowded_frames),
        Err(GifEncodingError::TooManyColors { frame_index: 1 })
    ));
}

#[test]
fn partly_transparent_pixels_fail_instead_of_rounding() {
    assert!(matches!(
        encode_gif(&[frame_with_colors([[200, 100, 50, 128]])]),
        Err(GifEncodingError::PartialTransparency { frame_index: 0 })
    ));
}

#[test]
fn the_preview_pairs_each_theme_and_loads_no_scripts_or_external_assets() {
    let preview_html = encode_preview_html("Blue galaxy <for> two themes & more", "blue");
    assert!(preview_html.contains("Blue galaxy &lt;for&gt; two themes &amp; more"));
    assert!(!preview_html.contains("<script"));
    assert!(!preview_html.contains("http://"));
    assert!(!preview_html.contains("https://"));
    assert!(preview_html.contains("#ffffff"));
    assert!(preview_html.contains("#0d1117"));
    assert_eq!(preview_html.matches("<section").count(), 2);
    let dark_start = preview_html.find("class=\"dark\"").unwrap();
    let light_section = &preview_html[..dark_start];
    let dark_section = &preview_html[dark_start..];
    assert!(light_section.contains("galaxy-light.gif"));
    assert!(light_section.contains("galaxy-light.png"));
    assert!(!light_section.contains("galaxy-dark."));
    assert!(dark_section.contains("galaxy-dark.gif"));
    assert!(dark_section.contains("galaxy-dark.png"));
    assert!(!dark_section.contains("galaxy-light."));
    assert_eq!(preview_html.matches("<img ").count(), 4);
    assert!(preview_html.contains("alt=\"The blue galaxy on a light background\""));
    assert!(preview_html.contains("alt=\"The blue galaxy still on a dark background\""));
    let export_size = format!("width=\"{STATIC_WIDTH_PIXELS}\" height=\"{STATIC_HEIGHT_PIXELS}\"");
    assert_eq!(preview_html.matches(&export_size).count(), 4);
}

#[test]
fn the_preview_alt_names_the_colorway_instead_of_the_default() {
    let preview_html = encode_preview_html("Red galaxy", "red");
    assert!(preview_html.contains("alt=\"The red galaxy on a light background\""));
    assert!(preview_html.contains("alt=\"The red galaxy still on a dark background\""));
    assert!(!preview_html.contains("blue galaxy"));
}

fn read_development() -> gitlaxy::galaxy::GalaxyDevelopment {
    let account_snapshot: AccountSnapshot =
        serde_json::from_str(include_str!("fixtures/account-with-history.json")).unwrap();
    derive_galaxy_development(&account_snapshot).unwrap()
}

fn two_theme_frames() -> Vec<GalaxyFrame> {
    let galaxy_development = read_development();
    vec![
        render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Dark),
        render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Light),
    ]
}

/// The 120 identical still frames of one loop and their GIF, rendered once per test run.
static ENCODED_STILL_LOOP: LazyLock<(Vec<GalaxyFrame>, Vec<u8>)> = LazyLock::new(|| {
    let still_frame = render_galaxy_still(&read_development(), Colorway::Blue, Theme::Dark);
    let galaxy_frames = vec![still_frame; 120];
    let gif_bytes = encode_gif(&galaxy_frames).unwrap();
    (galaxy_frames, gif_bytes)
});

/// A frame with one opaque pixel of each given color, in reading order.
fn frame_with_colors(colors: impl IntoIterator<Item = [u8; 4]>) -> GalaxyFrame {
    let mut rgba_pixels = vec![[0; 4]; FRAME_PIXEL_COUNT];
    for (pixel_channels, color) in rgba_pixels.iter_mut().zip(colors) {
        *pixel_channels = color;
    }
    GalaxyFrame::from_rgba_pixels(rgba_pixels)
}

fn distinct_colors(color_count: u32) -> impl Iterator<Item = [u8; 4]> {
    (0..color_count).map(|color_index| [color_index as u8, (color_index >> 8) as u8, 7, 255])
}

/// Repeat each native pixel twice in both dimensions, as the export does.
fn scale_to_export(galaxy_frame: &GalaxyFrame) -> Vec<[u8; 4]> {
    let native_pixels = galaxy_frame.as_rgba_bytes().as_chunks::<4>().0;
    (0..STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS)
        .map(|export_index| {
            let native_x = export_index % STATIC_WIDTH_PIXELS / 2;
            let native_y = export_index / STATIC_WIDTH_PIXELS / 2;
            native_pixels[(native_y * FRAME_WIDTH_PIXELS + native_x) as usize]
        })
        .collect()
}
