use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use chrono::{DateTime, NaiveDate};
use gitlaxy::collect::{GhCli, collect_account_snapshot};
use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{GalaxyDevelopment, calculate_stage, derive_galaxy_development};
use gitlaxy::history::AccountSnapshot;
use gitlaxy::output::{encode_gif, encode_png, encode_preview_html};
use gitlaxy::render::{GalaxyFrame, LOOP_FRAME_COUNT, render_galaxy_frame};
use gitlaxy::theme::Theme;

const USAGE: &str = "Usage: gitlaxy collect LOGIN [--output FILE]
       gitlaxy render SNAPSHOT --output-dir DIR [--colorway COLORWAY]
       gitlaxy --help | --version
Collect LOGIN's public GitHub snapshot as JSON through `gh api`: public owned and
contributed repositories, and daily contributions from the account's creation
through yesterday (UTC), the last complete day.
Writes to standard output, or replaces FILE only after collection succeeds.
Requires an authenticated GitHub CLI; in GitHub Actions, set GH_TOKEN to github.token.
Render a collected SNAPSHOT into DIR: galaxy-dark.png, galaxy-light.png,
galaxy-dark.gif, and galaxy-light.gif show the galaxy at its contribution stage in the
chosen colorway on the dark and light page backgrounds, and preview.html shows all four.
The files are replaced only after all of them render.
COLORWAY is blue (the default), red, orange, purple, green, or pink.
Errors go to stderr and exit 1; invalid usage exits 2.
Examples: gitlaxy collect octocat --output snapshot.json
          gitlaxy render snapshot.json --output-dir galaxy --colorway purple";

fn main() -> ExitCode {
    let program_arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let argument_texts: Vec<Option<&str>> = program_arguments
        .iter()
        .map(|argument| argument.to_str())
        .collect();
    match argument_texts.as_slice() {
        [Some("--help")] | [Some("collect" | "render"), Some("--help")] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        [Some("--version")] => {
            println!("gitlaxy {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [Some("collect"), Some(login)] if !login.starts_with('-') => run_collect(login, None),
        [Some("collect"), Some(login), Some("--output"), _] if !login.starts_with('-') => {
            run_collect(login, Some(Path::new(&program_arguments[3])))
        }
        [Some("render"), _, ..] => match parse_render_arguments(&program_arguments[1..]) {
            Some(render_arguments) => run_render(render_arguments),
            None => {
                eprintln!("{USAGE}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_collect(login: &str, output_path: Option<&Path>) -> ExitCode {
    let collection_result = collect_account_snapshot(&GhCli, login, current_utc_date())
        .map_err(|collection_error| collection_error.to_string())
        .and_then(|account_snapshot| {
            serde_json::to_string_pretty(&account_snapshot)
                .map_err(|serialize_error| serialize_error.to_string())
        })
        .and_then(|snapshot_json| write_snapshot(&format!("{snapshot_json}\n"), output_path));
    match collection_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("gitlaxy: {error_message}");
            ExitCode::FAILURE
        }
    }
}

/// The parsed shape of `gitlaxy render SNAPSHOT --output-dir DIR [--colorway COLORWAY]`.
struct RenderArguments<'a> {
    snapshot_path: &'a Path,
    output_directory: &'a Path,
    colorway: Colorway,
}

/// Parse render flags in either order, rejecting unknown, missing, or duplicate ones.
fn parse_render_arguments(program_arguments: &[OsString]) -> Option<RenderArguments<'_>> {
    let (snapshot_argument, flag_arguments) = program_arguments.split_first()?;
    if snapshot_argument.to_str()?.starts_with('-') {
        return None;
    }
    let mut output_directory = None;
    let mut colorway = None;
    let mut flag_index = 0;
    while flag_index < flag_arguments.len() {
        let flag = flag_arguments[flag_index].to_str()?;
        let value = flag_arguments.get(flag_index + 1)?;
        match flag {
            "--output-dir" if output_directory.is_none() => {
                output_directory = Some(Path::new(value));
            }
            "--colorway" if colorway.is_none() => {
                colorway = Some(Colorway::parse(value.to_str()?)?);
            }
            _ => return None,
        }
        flag_index += 2;
    }
    Some(RenderArguments {
        snapshot_path: Path::new(snapshot_argument),
        output_directory: output_directory?,
        colorway: colorway.unwrap_or_default(),
    })
}

fn run_render(arguments: RenderArguments<'_>) -> ExitCode {
    let RenderArguments {
        snapshot_path,
        output_directory,
        colorway,
    } = arguments;
    let render_result = render_day(snapshot_path, colorway).and_then(|rendered_day| {
        fs::create_dir_all(output_directory)
            .map_err(|create_error| format!("{}: {create_error}", output_directory.display()))?;
        let written_paths = publish_output_files(output_directory, &rendered_day.output_files)?;
        println!(
            "{}: wrote {}",
            rendered_day.caption,
            written_paths.join(", ")
        );
        Ok(())
    });
    match render_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("gitlaxy: {error_message}");
            ExitCode::FAILURE
        }
    }
}

/// The day's files, by name, with a caption naming the stage and observation date.
struct RenderedDay {
    caption: String,
    output_files: [(String, Vec<u8>); 5],
}

/// Render and encode both themes in memory, so none is written unless all succeed.
fn render_day(snapshot_path: &Path, colorway: Colorway) -> Result<RenderedDay, String> {
    let snapshot_json = fs::read_to_string(snapshot_path)
        .map_err(|read_error| format!("{}: {read_error}", snapshot_path.display()))?;
    let account_snapshot: AccountSnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|parse_error| format!("{}: {parse_error}", snapshot_path.display()))?;
    let galaxy_development = derive_galaxy_development(&account_snapshot)
        .map_err(|development_error| development_error.to_string())?;
    let dark_frames = render_loop(&galaxy_development, colorway, Theme::Dark);
    let light_frames = render_loop(&galaxy_development, colorway, Theme::Light);
    let dark_gif = encode_gif(&dark_frames).map_err(|encoding_error| encoding_error.to_string())?;
    let light_gif =
        encode_gif(&light_frames).map_err(|encoding_error| encoding_error.to_string())?;
    let dark_png =
        encode_png(&dark_frames[0]).map_err(|encoding_error| encoding_error.to_string())?;
    let light_png =
        encode_png(&light_frames[0]).map_err(|encoding_error| encoding_error.to_string())?;
    let caption = format!(
        "{} galaxy at stage {} for {}",
        colorway.name(),
        calculate_stage(galaxy_development.total_contributions),
        galaxy_development.observed_on
    );
    let preview_html = encode_preview_html(&caption, colorway.name());
    Ok(RenderedDay {
        caption,
        output_files: [
            (galaxy_file_name(Theme::Dark, "png"), dark_png),
            (galaxy_file_name(Theme::Light, "png"), light_png),
            (galaxy_file_name(Theme::Dark, "gif"), dark_gif),
            (galaxy_file_name(Theme::Light, "gif"), light_gif),
            ("preview.html".to_owned(), preview_html.into_bytes()),
        ],
    })
}

/// The output file name for one theme and format.
fn galaxy_file_name(theme: Theme, extension: &str) -> String {
    format!("galaxy-{}.{extension}", theme.name())
}

/// Render every frame of one theme's seamless loop, in memory.
fn render_loop(
    galaxy_development: &GalaxyDevelopment,
    colorway: Colorway,
    theme: Theme,
) -> Vec<GalaxyFrame> {
    (0..LOOP_FRAME_COUNT)
        .map(|frame_index| render_galaxy_frame(galaxy_development, colorway, theme, frame_index))
        .collect()
}

// GitHub dates its contribution calendar in UTC.
fn current_utc_date() -> NaiveDate {
    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is after 1970")
        .as_secs();
    DateTime::from_timestamp(
        i64::try_from(seconds_since_epoch).expect("the system clock is in range"),
        0,
    )
    .expect("the system clock is in range")
    .date_naive()
}

/// Write to standard output, or replace the file.
fn write_snapshot(snapshot_json: &str, output_path: Option<&Path>) -> Result<(), String> {
    let Some(output_path) = output_path else {
        return io::stdout()
            .write_all(snapshot_json.as_bytes())
            .map_err(|write_error| format!("standard output: {write_error}"));
    };
    replace_file(output_path, snapshot_json.as_bytes())
}

/// Replace the file through a sibling temporary file named after it, so a failed write
/// never leaves a partial file in place.
fn replace_file(output_path: &Path, file_bytes: &[u8]) -> Result<(), String> {
    let mut temporary_name = output_path.file_name().unwrap_or_default().to_owned();
    temporary_name.push(".tmp");
    let temporary_path = output_path.with_file_name(temporary_name);
    fs::write(&temporary_path, file_bytes)
        .and_then(|()| fs::rename(&temporary_path, output_path))
        .map_err(|write_error| {
            let _ = fs::remove_file(&temporary_path);
            format!("{}: {write_error}", output_path.display())
        })
}

/// Publish every file as one unit: stage a temporary for each, move the old files aside,
/// rename the temporaries into place, and on any failure restore the old files and remove
/// the rest. On success the backups are deleted.
fn publish_output_files(
    output_directory: &Path,
    output_files: &[(String, Vec<u8>)],
) -> Result<Vec<String>, String> {
    let staged_paths: Vec<PathBuf> = output_files
        .iter()
        .map(|(file_name, _)| output_directory.join(format!("{file_name}.tmp")))
        .collect();
    let roll_back = |moved_aside: &[(PathBuf, PathBuf)], published: &[PathBuf]| {
        for output_path in published {
            let _ = fs::remove_file(output_path);
        }
        for (output_path, backup_path) in moved_aside.iter().rev() {
            let _ = fs::rename(backup_path, output_path);
        }
        for staged_path in &staged_paths {
            let _ = fs::remove_file(staged_path);
        }
    };

    let mut moved_aside: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut published: Vec<PathBuf> = Vec::new();
    for ((file_name, file_bytes), staged_path) in output_files.iter().zip(&staged_paths) {
        if let Err(write_error) = fs::write(staged_path, file_bytes) {
            roll_back(&moved_aside, &published);
            return Err(format!("{}: {write_error}", staged_path.display()));
        }
        let output_path = output_directory.join(file_name);
        match fs::symlink_metadata(&output_path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let backup_path = output_directory.join(format!("{file_name}.bak"));
                if let Err(rename_error) = fs::rename(&output_path, &backup_path) {
                    roll_back(&moved_aside, &published);
                    return Err(format!("{}: {rename_error}", output_path.display()));
                }
                moved_aside.push((output_path.clone(), backup_path));
            }
            Ok(_) => {
                roll_back(&moved_aside, &published);
                return Err(format!(
                    "{}: cannot replace a path that is not a regular file",
                    output_path.display()
                ));
            }
            Err(metadata_error) if metadata_error.kind() == io::ErrorKind::NotFound => {}
            Err(metadata_error) => {
                roll_back(&moved_aside, &published);
                return Err(format!("{}: {metadata_error}", output_path.display()));
            }
        }
        if let Err(rename_error) = fs::rename(staged_path, &output_path) {
            roll_back(&moved_aside, &published);
            return Err(format!("{}: {rename_error}", output_path.display()));
        }
        published.push(output_path);
    }
    for (_, backup_path) in &moved_aside {
        fs::remove_file(backup_path)
            .map_err(|remove_error| format!("{}: {remove_error}", backup_path.display()))?;
    }
    Ok(published
        .iter()
        .map(|output_path| output_path.display().to_string())
        .collect())
}
