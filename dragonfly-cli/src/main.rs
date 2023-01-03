use anyhow::Context;
use clap::{Parser, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use log::debug;
use serde::{Deserialize, Serialize};
use std::env::temp_dir;
use std::path::Path;
use std::time::Duration;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use strum::{Display, EnumString};
use which::which;

static FFMPEG_BINARY_NAME: &str = if cfg!(target_os = "windows") {
    "ffmpeg.exe"
} else {
    "ffmpeg"
};

static FFPROBE_BINARY_NAME: &str = if cfg!(target_os = "windows") {
    "ffprobe.exe"
} else {
    "ffprobe"
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(help = "Path to input image or video")]
    input_path: PathBuf,
    #[arg(help = "Processing mode",
        default_value_t = CliMode::Both,
        long
    )]
    mode: CliMode,
    #[arg(
        help = "Output horizontal field of view in degrees",
        default_value = "90.0",
        long
    )]
    h_fov: f32,
    #[arg(
        help = "Output vertical field of view in degrees",
        default_value = "45.0",
        long
    )]
    v_fov: f32,
    #[arg(
        help = "Input horizontal field of view in degrees",
        default_value = "360.0",
        long
    )]
    ih_fov: f32,
    #[arg(
        help = "Input vertical field of view in degrees",
        default_value = "180.0",
        long
    )]
    iv_fov: f32,
    #[arg(help = "Interpolation method",
        default_value_t = CliInterpolation::Linear,
        long
    )]
    interpolation: CliInterpolation,
    #[arg(
        help = "Retry the encoding step and re-use the frame extraction from the latest run",
        long
    )]
    retry: bool,
    #[arg(
        help = "Number of concurrent instances of ffmpeg to run for frame extraction",
        long,
        default_value = "8"
    )]
    j: usize,
}

#[derive(ValueEnum, Display, Debug, Clone, EnumString)]
#[strum(serialize_all = "lowercase")]
enum CliInterpolation {
    Near,
    Linear,
    Cubic,
    Lanczos,
    Spline16,
    Lagrange9,
    Gaussian,
    Mitchell,
}

#[derive(ValueEnum, Display, Debug, Clone, EnumString, PartialEq)]
#[strum(serialize_all = "lowercase")]
enum CliMode {
    Extract,
    Encode,
    Both,
}

#[derive(Debug, Serialize, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStreamOutput>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FfprobeStreamOutput {
    width: i32,
    height: i32,
}

fn extract_frames(
    input_path_str: &str,
    frame_count: usize,
    image_extraction_path: &Path,
    cli: &Cli,
    progress_callback: impl Fn(usize, usize),
) -> anyhow::Result<()> {
    // Fetch the input pixel resolution
    let ffprobe_child = Command::new(FFPROBE_BINARY_NAME)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "json=compact=1",
            input_path_str,
        ])
        .stdout(Stdio::piped())
        .spawn()?;
    let ffprobe_output = ffprobe_child.wait_with_output()?;
    let ffprobe_output = serde_json::from_slice::<FfprobeOutput>(&ffprobe_output.stdout)?;
    let ffprobe_stream_output = ffprobe_output.streams.first().with_context(|| {
        format!(
            "Failed to find stream output in ffprobe output: {:?}",
            ffprobe_output
        )
    })?;
    // Calculate the output image resolution based on the input and output FOVs and initial input resolution
    let ih_fov = cli.ih_fov;
    let iv_fov = cli.iv_fov;
    let oh_fov = cli.h_fov;
    let ov_fov = cli.v_fov;
    let h_ratio = oh_fov / ih_fov;
    let v_ratio = ov_fov / iv_fov;
    let output_width = (ffprobe_stream_output.width as f32 * h_ratio) as i32;
    let output_height = (ffprobe_stream_output.height as f32 * v_ratio) as i32;

    let mut tasks = Vec::with_capacity(cli.j);
    // Extract frames
    for frame in 0..frame_count {
        // Want to exclude +180.0 from the yaw calculation to avoid having duplicate starting and ending frames
        //let yaw = -180.0 + 360.0 * (frame as f32 / (frames - 1) as f32);
        let yaw = -180.0 + 360.0 * (frame as f32 / frame_count as f32);
        let pitch = 0.0;
        let roll = 0.0;
        let output_path = image_extraction_path.join(format!("frame_{:08}.jpg", frame));
        let output_path_str = output_path.to_str().with_context(|| {
            format!("Failed to convert output path to string: {:?}", output_path)
        })?;
        let mut ffmpeg_cmd = Command::new("ffmpeg");
        ffmpeg_cmd.args([
            // Quiet output
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostats",
            // Input file
            "-i",
            input_path_str,
            // Video filter arguments
            // See https://ffmpeg.org/ffmpeg-filters.html#v360
            "-vf",
            &format!(
                "v360=e:flat:yaw={}:pitch={}:roll={}:ih_fov={}:iv_fov={}:h_fov={}:v_fov={}:interp={}:w={}:h={}",
                yaw,
                pitch,
                roll,
                ih_fov,
                iv_fov,
                oh_fov,
                ov_fov,
                cli.interpolation,
                output_width,
                output_height,
            ),
            // Output file
            // https://ffmpeg.org/ffmpeg-formats.html#image2-1
            "-f",
            "image2",
            "-frames:v",
            "1",
            "-update",
            "1",
            "-y",
            output_path_str,
        ]);
        debug!("Spawning command: {:?}", &ffmpeg_cmd);
        let ffmpeg_child = ffmpeg_cmd.stdout(Stdio::piped()).spawn()?;
        tasks.push(ffmpeg_child);
        // Wait for tasks to finish if we have reached the maximum number of concurrent tasks
        if tasks.len() == tasks.capacity() {
            for task in tasks.iter_mut() {
                task.wait()?;
            }
            tasks.clear();
        }
        progress_callback(frame, frame_count);
    }
    Ok(())
}

fn encode_frames(
    output: &str,
    image_extraction_path: &Path,
    input_frames_per_second: f32,
) -> anyhow::Result<()> {
    // Encode output
    let frame_path_template = image_extraction_path.join("frame_%08d.jpg");
    let frame_path_template_str = frame_path_template.to_str().with_context(|| {
        format!(
            "Failed to convert output path to string: {:?}",
            image_extraction_path.join("frame_%08d.jpg")
        )
    })?;
    let mut ffmpeg_cmd = Command::new("ffmpeg");
    ffmpeg_cmd.args([
        // Quiet output
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostats",
        // Input file
        "-framerate",
        input_frames_per_second.to_string().as_str(),
        "-i",
        frame_path_template_str,
        "-c:v",
        "libx264",
        "-r",
        "24",
        "-y",
        output,
    ]);
    debug!("Spawning command: {:?}", &ffmpeg_cmd);
    let mut ffmpeg_child = ffmpeg_cmd.stdout(Stdio::piped()).spawn()?;
    ffmpeg_child.wait()?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let cli = Cli::parse();
    let stdout = console::Term::stdout();
    let stderr = console::Term::stderr();
    // Ensure all required binaries are on the PATH
    let required_binaries = [FFMPEG_BINARY_NAME, FFPROBE_BINARY_NAME];
    for required_binary in required_binaries {
        if which(FFMPEG_BINARY_NAME).is_err() {
            stderr.write_line(&format!(
                "\"{}\" not found, please install it at https://ffmpeg.org/",
                required_binary
            ))?;
            std::process::exit(exitcode::UNAVAILABLE);
        }
    }

    let steps = match cli.mode {
        CliMode::Extract => 1,
        CliMode::Encode => 1,
        CliMode::Both => 2,
    };

    // The image extraction path is a temporary directory if we are extracting
    // Otherwise, we use the input_path specified in the CLI
    let image_extraction_path = if cli.mode == CliMode::Extract || cli.mode == CliMode::Both {
        let tmp_dir_path = temp_dir();
        tmp_dir_path.join(format!(
            "com.jshrake.dragonfly-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        ))
    } else {
        cli.input_path.clone()
    };

    let frame_count: usize = 360 * 48;
    let revolutions_per_minute = 10.0 / 60.0;
    let input_frames_per_second = frame_count as f32 / (60.0 * revolutions_per_minute);

    if cli.mode == CliMode::Extract || cli.mode == CliMode::Both {
        debug!("Creating project directory: {:?}", image_extraction_path);
        std::fs::create_dir_all(&image_extraction_path)?;
        let input_path_str = cli.input_path.to_str().with_context(|| {
            format!(
                "Failed to convert input path to string: {:?}",
                cli.input_path
            )
        })?;
        stdout.write_line(&format!(
            "[1/{}] Extracting {} frames from {} to {:?}",
            steps, frame_count, input_path_str, image_extraction_path
        ))?;
        let pb = ProgressBar::new(frame_count as u64);
        extract_frames(
            input_path_str,
            frame_count,
            &image_extraction_path,
            &cli,
            |_, _| {
                pb.inc(1);
            },
        )?;
        pb.finish_and_clear();
    }

    if cli.mode == CliMode::Encode || cli.mode == CliMode::Both {
        let output = "output.mp4";
        stdout.write_line(&format!(
            "[{}/{}] Encoding {} frames to {}",
            std::cmp::min(steps, 2),
            steps,
            frame_count,
            output
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
        encode_frames(output, &image_extraction_path, input_frames_per_second)?;
        pb.finish_and_clear();
    }

    std::process::exit(exitcode::OK);
}
