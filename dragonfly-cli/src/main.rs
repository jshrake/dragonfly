use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::env::temp_dir;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;
use std::time::Duration;
use which::which;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct DragonflyCli {
    #[command(subcommand)]
    subcommand: DragonflySubCommand,
}

/// Extract rectilinear frames from a equirectangular (360) image
#[derive(Subcommand, Debug)]
enum DragonflySubCommand {
    /// Extract rectilinear frames from a equirectangular (360) image, then encode them into a seamless video (mp4, webm, gif)
    Run {
        #[arg(help = "Path to input 360 image")]
        input_path: PathBuf,
        #[command(flatten)]
        extract_args: dragonfly::ExtractFramesDescriptor,
        #[command(flatten)]
        encode_args: dragonfly::EncodeFramesDescriptor,
        #[arg(help = "Path to output media file", default_value = "output.mp4")]
        output_path: PathBuf,
    },
    /// Extract rectilinear frames from a equirectangular (360) image
    Extract {
        #[arg(help = "Path to input 360 image")]
        input_path: PathBuf,
        #[command(flatten)]
        args: dragonfly::ExtractFramesDescriptor,
        #[arg(help = "Output directory for extracted frames, defaults to a temporary directory")]
        extract_path: Option<PathBuf>,
    },
    /// Encode extracted rectilinear frames into a seamless video (mp4, webm, gif)
    Encode {
        #[arg(help = "Path to directory containing extracted images")]
        extract_path: Option<PathBuf>,
        #[command(flatten)]
        args: dragonfly::EncodeFramesDescriptor,
        #[arg(help = "Path to output media file", default_value = "output.mp4")]
        output_path: PathBuf,
    },
}

/// Creates a temporary directory to hold the extracted frames
/// Returns the path to the directory
fn create_tmp_extract_dir() -> anyhow::Result<PathBuf> {
    let tmp_dir_path = temp_dir();
    let extraction_path = tmp_dir_path.join(format!(
        "com.jshrake.dragonfly-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    ));
    std::fs::create_dir_all(&extraction_path)?;
    Ok(extraction_path)
}

fn store_extract_dir(dir: &PathBuf) -> anyhow::Result<()> {
    let tmp_dir_path = temp_dir();
    let file_path = tmp_dir_path.join(".dragonfly");
    let mut file = File::create(file_path)?;
    file.write_all(dir.as_os_str().as_bytes())?;
    Ok(())
}

fn retrieve_extract_dir() -> anyhow::Result<PathBuf> {
    let tmp_dir_path = temp_dir();
    let file_path = tmp_dir_path.join(".dragonfly");
    let mut file = File::open(file_path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf.into())
}

fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let cli = DragonflyCli::parse();
    let stdout = console::Term::stdout();
    let stderr = console::Term::stderr();
    // Ensure all required binaries are on the PATH
    let required_binaries = [
        dragonfly::FFMPEG_BINARY_PATH.as_os_str(),
        dragonfly::FFPROBE_BINARY_PATH.as_os_str(),
    ];
    for required_binary in required_binaries {
        if let Some(binary_name) = required_binary.to_str() {
            if which(binary_name).is_err() {
                stderr.write_line(&format!(
                    "\"{}\" not found, please install it at https://ffmpeg.org/",
                    binary_name
                ))?;
                std::process::exit(exitcode::UNAVAILABLE);
            }
        }
    }

    match cli.subcommand {
        DragonflySubCommand::Run { .. } => {
            todo!();
        }
        DragonflySubCommand::Extract {
            input_path,
            extract_path,
            args,
        } => {
            // The extract path was either specified by the user, or we need to create a temporary directory
            let extract_path = if let Some(extract_path) = extract_path {
                extract_path
            } else {
                create_tmp_extract_dir()?
            };
            // Store the extract path so we can use it in future encode commands without the user having to specify it
            if store_extract_dir(&extract_path).is_err() {
                stderr.write_line(
                    "Unexpectedly failed to store extract path. Attempting to continue...",
                )?;
            }

            stdout.write_line(&format!(
                "Extracting {} frames from {:?} to {:?}",
                args.frame_count, input_path, extract_path
            ))?;
            let pb = ProgressBar::new(args.frame_count as u64);
            dragonfly::extract_frames(
                &input_path,
                &extract_path,
                &args,
                Some(|_, _| {
                    pb.inc(1);
                }),
            )?;
            pb.finish_and_clear();
        }
        DragonflySubCommand::Encode {
            extract_path,
            output_path,
            args,
        } => {
            // The user either specified the extract path explicitly, or we will attempt to find the last extract path used
            let extract_path = if let Some(extract_path) = extract_path {
                extract_path
            } else {
                // TODO: If this returns an error, we should tell the user rather than exit the process
                if let Ok(extract_path) = retrieve_extract_dir() {
                    extract_path
                } else {
                    stderr.write_line(
                        "Unable to find the last extract path. Please specify it explicitly.",
                    )?;
                    std::process::exit(exitcode::USAGE);
                }
            };
            stdout.write_line(&format!(
                "Encoding frames from {:?} to {:?}",
                extract_path, output_path
            ))?;
            let pb = ProgressBar::new_spinner();
            pb.enable_steady_tick(Duration::from_millis(120));
            pb.set_style(
                ProgressStyle::with_template("{spinner:.blue} {msg}")
                    .unwrap()
                    // For more spinners check out the cli-spinners project:
                    // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
                    .tick_strings(&[
                        "▹▹▹▹▹",
                        "▸▹▹▹▹",
                        "▹▸▹▹▹",
                        "▹▹▸▹▹",
                        "▹▹▹▸▹",
                        "▹▹▹▹▸",
                        "▪▪▪▪▪",
                    ]),
            );
            pb.set_message("Encoding...");
            dragonfly::encode_frames(&output_path, &extract_path, &args)?;
            pb.finish_and_clear();
        }
    }

    std::process::exit(exitcode::OK);
}
