use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::derive_galaxy_development;
use gitlaxy::history::AccountSnapshot;
use gitlaxy::output::{STATIC_HEIGHT_PIXELS, STATIC_WIDTH_PIXELS, encode_gif, encode_png};
use gitlaxy::render::{GalaxyFrame, LOOP_FRAME_COUNT, render_galaxy_frame, render_galaxy_still};
use gitlaxy::theme::Theme;

const HISTORY_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/account-with-history.json"
);
const MISSING_SNAPSHOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/missing-snapshot.json"
);
const OUTPUT_FILE_NAMES: [&str; 5] = [
    "galaxy-dark.png",
    "galaxy-light.png",
    "galaxy-dark.gif",
    "galaxy-light.gif",
    "preview.html",
];

#[test]
fn render_writes_the_five_galaxy_files_as_the_renderer_draws_them() {
    // No --colorway means the default blue palette, compared against below.
    let output_directory = fresh_directory("render-files");
    let command_output = run_gitlaxy(&[
        "render",
        HISTORY_FIXTURE,
        "--output-dir",
        output_directory.to_str().unwrap(),
    ]);
    assert_eq!(
        command_output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&command_output.stderr)
    );
    let written_names: BTreeSet<String> = fs::read_dir(&output_directory)
        .unwrap()
        .map(|directory_entry| directory_entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        written_names,
        OUTPUT_FILE_NAMES.map(str::to_owned).into_iter().collect()
    );

    let galaxy_development = read_fixture_development();
    for (theme, file_stem) in [(Theme::Dark, "galaxy-dark"), (Theme::Light, "galaxy-light")] {
        let loop_frames: Vec<GalaxyFrame> = (0..LOOP_FRAME_COUNT)
            .map(|frame_index| {
                render_galaxy_frame(&galaxy_development, Colorway::Blue, theme, frame_index)
            })
            .collect();
        assert_eq!(
            fs::read(output_directory.join(format!("{file_stem}.png"))).unwrap(),
            encode_png(&loop_frames[0]).unwrap()
        );
        assert_eq!(
            fs::read(output_directory.join(format!("{file_stem}.gif"))).unwrap(),
            encode_gif(&loop_frames).unwrap()
        );
    }
    let preview_html = fs::read_to_string(output_directory.join("preview.html")).unwrap();
    let observation_date = galaxy_development.observed_on.to_string();
    assert!(preview_html.contains(&observation_date));
    let summary_line = String::from_utf8_lossy(&command_output.stdout);
    assert!(summary_line.contains(&observation_date));
    assert!(summary_line.contains("stage 3"));
}

#[test]
fn both_themes_are_512_square_on_their_exact_opaque_background() {
    let output_directory = fresh_directory("render-themes");
    let command_output = run_gitlaxy(&[
        "render",
        HISTORY_FIXTURE,
        "--output-dir",
        output_directory.to_str().unwrap(),
    ]);
    assert_eq!(command_output.status.code(), Some(0));
    for theme in [Theme::Dark, Theme::Light] {
        let file_stem = format!("galaxy-{}", theme.name());
        let png_pixels =
            decode_png(&fs::read(output_directory.join(format!("{file_stem}.png"))).unwrap());
        assert_eq!(
            png_pixels.len(),
            (STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS) as usize
        );
        assert_eq!(png_pixels[0], theme.background());
        assert!(png_pixels.iter().all(|pixel| pixel[3] == 255));
        assert!(png_pixels.iter().any(|pixel| *pixel != theme.background()));

        let gif_bytes = fs::read(output_directory.join(format!("{file_stem}.gif"))).unwrap();
        let mut gif_options = gif::DecodeOptions::new();
        gif_options.set_color_output(gif::ColorOutput::RGBA);
        let mut gif_decoder = gif_options.read_info(gif_bytes.as_slice()).unwrap();
        assert_eq!(
            (
                u32::from(gif_decoder.width()),
                u32::from(gif_decoder.height())
            ),
            (STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS)
        );
        assert_eq!(gif_decoder.repeat(), gif::Repeat::Infinite);
        let mut frame_count = 0;
        let mut total_hundredths = 0;
        let mut first_frame_pixels = Vec::new();
        while let Some(gif_frame) = gif_decoder.read_next_frame().unwrap() {
            frame_count += 1;
            total_hundredths += u32::from(gif_frame.delay);
            assert_eq!(
                (u32::from(gif_frame.width), u32::from(gif_frame.height)),
                (STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS)
            );
            if frame_count == 1 {
                first_frame_pixels.extend_from_slice(&gif_frame.buffer);
            }
        }
        assert_eq!(frame_count, LOOP_FRAME_COUNT as usize);
        assert_eq!(total_hundredths, 800);
        assert_eq!(first_frame_pixels.as_slice(), png_pixels.as_flattened());
    }
}

#[test]
fn the_preview_shows_one_light_and_one_dark_pair_without_scripts() {
    let output_directory = fresh_directory("render-preview");
    let command_output = run_gitlaxy(&[
        "render",
        HISTORY_FIXTURE,
        "--output-dir",
        output_directory.to_str().unwrap(),
    ]);
    assert_eq!(command_output.status.code(), Some(0));
    let preview_html = fs::read_to_string(output_directory.join("preview.html")).unwrap();
    assert_eq!(preview_html.matches("<section").count(), 2);
    assert!(!preview_html.contains("<script"));
    assert!(!preview_html.contains("http://"));
    assert!(!preview_html.contains("https://"));
    assert!(preview_html.contains("#ffffff"));
    assert!(preview_html.contains("#0d1117"));
    let dark_start = preview_html.find("class=\"dark\"").unwrap();
    let light_section = &preview_html[..dark_start];
    let dark_section = &preview_html[dark_start..];
    assert!(light_section.contains("galaxy-light.gif"));
    assert!(light_section.contains("galaxy-light.png"));
    assert!(!light_section.contains("galaxy-dark."));
    assert!(dark_section.contains("galaxy-dark.gif"));
    assert!(dark_section.contains("galaxy-dark.png"));
    assert!(!dark_section.contains("galaxy-light."));
    assert!(preview_html.contains(&read_fixture_development().observed_on.to_string()));
}

#[test]
fn repeated_runs_write_byte_identical_files() {
    let first_directory = fresh_directory("render-repeat-first");
    let second_directory = fresh_directory("render-repeat-second");
    for output_directory in [&first_directory, &second_directory] {
        let command_output = run_gitlaxy(&[
            "render",
            HISTORY_FIXTURE,
            "--output-dir",
            output_directory.to_str().unwrap(),
        ]);
        assert_eq!(command_output.status.code(), Some(0));
    }
    for file_name in OUTPUT_FILE_NAMES {
        assert_eq!(
            fs::read(first_directory.join(file_name)).unwrap(),
            fs::read(second_directory.join(file_name)).unwrap(),
            "{file_name} changed between runs"
        );
    }
}

#[test]
fn a_failed_render_exits_one_and_keeps_the_previous_files() {
    let output_directory = fresh_directory("render-failure");
    for file_name in OUTPUT_FILE_NAMES {
        fs::write(
            output_directory.join(file_name),
            format!("previous {file_name}"),
        )
        .unwrap();
    }
    let command_output = run_gitlaxy(&[
        "render",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/account.json"),
        "--output-dir",
        output_directory.to_str().unwrap(),
    ]);
    assert_eq!(command_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&command_output.stderr).starts_with("gitlaxy: "));
    for file_name in OUTPUT_FILE_NAMES {
        assert_eq!(
            fs::read_to_string(output_directory.join(file_name)).unwrap(),
            format!("previous {file_name}")
        );
    }
    assert_eq!(fs::read_dir(&output_directory).unwrap().count(), 5);
}

#[test]
fn render_usage_errors_exit_two_and_help_exits_zero() {
    for invalid_arguments in [
        &["render"][..],
        &["render", "--output-dir", "galaxy"],
        &["render", HISTORY_FIXTURE],
        &[
            "render",
            HISTORY_FIXTURE,
            "--output-dir",
            "galaxy",
            "--body",
            "black",
        ],
    ] {
        let command_output = run_gitlaxy(invalid_arguments);
        assert_eq!(
            command_output.status.code(),
            Some(2),
            "{invalid_arguments:?}"
        );
        assert!(String::from_utf8_lossy(&command_output.stderr).starts_with("Usage:"));
    }
    let help_output = run_gitlaxy(&["render", "--help"]);
    assert_eq!(help_output.status.code(), Some(0));
    let help_text = String::from_utf8_lossy(&help_output.stdout);
    assert!(help_text.contains("render SNAPSHOT"));
    assert!(help_text.contains("galaxy-dark.png"));
    assert!(!help_text.contains("koi"));
}

#[test]
fn every_colorway_name_is_accepted_in_either_flag_order() {
    for (index, colorway) in Colorway::ALL.into_iter().enumerate() {
        let output_directory =
            fresh_directory(&format!("render-colorway-accepted-{}", colorway.name()));
        let output_directory = output_directory.to_str().unwrap();
        // A missing snapshot reaches rendering, so exit 1 proves the flags parsed.
        let command_output = if index % 2 == 0 {
            run_gitlaxy(&[
                "render",
                MISSING_SNAPSHOT,
                "--output-dir",
                output_directory,
                "--colorway",
                colorway.name(),
            ])
        } else {
            run_gitlaxy(&[
                "render",
                MISSING_SNAPSHOT,
                "--colorway",
                colorway.name(),
                "--output-dir",
                output_directory,
            ])
        };
        assert_eq!(
            command_output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&command_output.stderr)
        );
        assert!(!String::from_utf8_lossy(&command_output.stderr).starts_with("Usage:"));
    }
}

#[test]
fn invalid_render_colorways_are_usage_errors() {
    let output_directory = fresh_directory("render-colorway-errors");
    let output_directory = output_directory.to_str().unwrap();
    for invalid_arguments in [
        &[
            "render",
            HISTORY_FIXTURE,
            "--output-dir",
            output_directory,
            "--colorway",
        ][..],
        &[
            "render",
            HISTORY_FIXTURE,
            "--output-dir",
            output_directory,
            "--colorway",
            "teal",
        ],
        &[
            "render",
            HISTORY_FIXTURE,
            "--output-dir",
            output_directory,
            "--colorway",
            "blue",
            "--colorway",
            "red",
        ],
        &[
            "render",
            HISTORY_FIXTURE,
            "--output-dir",
            output_directory,
            "--output-dir",
            output_directory,
        ],
        &[
            "render",
            HISTORY_FIXTURE,
            "--colorway",
            "blue",
            "--body",
            "blue",
            "--output-dir",
            output_directory,
        ],
    ] {
        let command_output = run_gitlaxy(invalid_arguments);
        assert_eq!(
            command_output.status.code(),
            Some(2),
            "{invalid_arguments:?}"
        );
        assert!(String::from_utf8_lossy(&command_output.stderr).starts_with("Usage:"));
    }
}

#[test]
fn a_non_blue_colorway_renders_both_themes_and_keeps_the_output_shape() {
    let output_directory = fresh_directory("render-colorway-purple");
    let command_output = run_gitlaxy(&[
        "render",
        HISTORY_FIXTURE,
        "--colorway",
        "purple",
        "--output-dir",
        output_directory.to_str().unwrap(),
    ]);
    assert_eq!(
        command_output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&command_output.stderr)
    );
    let written_names: BTreeSet<String> = fs::read_dir(&output_directory)
        .unwrap()
        .map(|directory_entry| directory_entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        written_names,
        OUTPUT_FILE_NAMES.map(str::to_owned).into_iter().collect()
    );

    let galaxy_development = read_fixture_development();
    for theme in [Theme::Dark, Theme::Light] {
        let file_stem = format!("galaxy-{}", theme.name());
        let png_pixels =
            decode_png(&fs::read(output_directory.join(format!("{file_stem}.png"))).unwrap());
        assert_eq!(
            png_pixels.len(),
            (STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS) as usize
        );
        assert_eq!(png_pixels[0], theme.background());
        assert!(png_pixels.iter().all(|pixel| pixel[3] == 255));

        let gif_bytes = fs::read(output_directory.join(format!("{file_stem}.gif"))).unwrap();
        let mut gif_options = gif::DecodeOptions::new();
        gif_options.set_color_output(gif::ColorOutput::RGBA);
        let mut gif_decoder = gif_options.read_info(gif_bytes.as_slice()).unwrap();
        let mut frame_count = 0;
        while gif_decoder.read_next_frame().unwrap().is_some() {
            frame_count += 1;
        }
        assert_eq!(frame_count, LOOP_FRAME_COUNT as usize);

        let blue_frame = render_galaxy_still(&galaxy_development, Colorway::Blue, theme);
        assert_ne!(
            dominant_color(&png_pixels, theme.background()),
            dominant_color(&rgba_pixels(&blue_frame), theme.background()),
            "purple did not replace blue on {theme:?}"
        );
    }
    let preview_html = fs::read_to_string(output_directory.join("preview.html")).unwrap();
    assert!(preview_html.contains("purple galaxy"));
}

#[test]
fn version_prints_the_manifest_version_and_exits_zero() {
    let version_output = run_gitlaxy(&["--version"]);
    assert_eq!(version_output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&version_output.stdout),
        format!("gitlaxy {}\n", env!("CARGO_PKG_VERSION"))
    );
}

fn read_fixture_development() -> gitlaxy::galaxy::GalaxyDevelopment {
    let account_snapshot: AccountSnapshot =
        serde_json::from_str(&fs::read_to_string(HISTORY_FIXTURE).unwrap()).unwrap();
    derive_galaxy_development(&account_snapshot).unwrap()
}

/// Decode a PNG into row-major RGBA pixels.
fn decode_png(png_bytes: &[u8]) -> Vec<[u8; 4]> {
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
    png_buffer.as_chunks::<4>().0.to_vec()
}

fn rgba_pixels(galaxy_frame: &GalaxyFrame) -> Vec<[u8; 4]> {
    galaxy_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|channels| {
            let [red, green, blue, alpha] = channels;
            [*red, *green, *blue, *alpha]
        })
        .collect()
}

/// The most frequent pixel that is clearly distinct from the background.
fn dominant_color(pixels: &[[u8; 4]], background: [u8; 4]) -> [u8; 4] {
    let mut counts = BTreeMap::new();
    for pixel in pixels {
        if pixel_distance(*pixel, background) > 24 * 24 {
            *counts.entry(*pixel).or_insert(0usize) += 1;
        }
    }
    *counts
        .iter()
        .max_by_key(|(_, count)| **count)
        .expect("a galaxy has pixels away from its background")
        .0
}

fn pixel_distance(left: [u8; 4], right: [u8; 4]) -> u32 {
    left.into_iter()
        .zip(right)
        .map(|(left_channel, right_channel)| u32::from(left_channel.abs_diff(right_channel)).pow(2))
        .sum()
}

/// An empty directory of the given name under Cargo's scratch space for tests.
fn fresh_directory(directory_name: &str) -> PathBuf {
    let directory_path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(directory_name);
    let _ = fs::remove_dir_all(&directory_path);
    fs::create_dir_all(&directory_path).unwrap();
    directory_path
}

fn run_gitlaxy(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gitlaxy"))
        .args(arguments)
        .output()
        .unwrap()
}
