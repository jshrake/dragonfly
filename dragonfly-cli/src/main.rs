use anyhow::Context;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use log::debug;
use serde::{Deserialize, Serialize};
use std::env::temp_dir;
use std::io::Write;
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
    #[arg(
        help = "Output horizontal field of view in degrees",
        default_value = "90.0"
    )]
    h_fov: f32,
    #[arg(
        help = "Output vertical field of view in degrees",
        default_value = "45.0"
    )]
    v_fov: f32,
    #[arg(
        help = "Input horizontal field of view in degrees",
        default_value = "360.0"
    )]
    ih_fov: f32,
    #[arg(
        help = "Input vertical field of view in degrees",
        default_value = "180.0"
    )]
    iv_fov: f32,
    #[arg(help = "Interpolation method", default_value_t = Interpolation::Linear)]
    interpolation: Interpolation,
}

#[derive(ValueEnum, Display, Debug, Clone, EnumString)]
#[strum(serialize_all = "lowercase")]
enum Interpolation {
    Near,
    Linear,
    Cubic,
    Lanczos,
    Spline16,
    Lagrange9,
    Gaussian,
    Mitchell,
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
            cli.input_path.to_str().unwrap(),
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

    let input_path_str = cli.input_path.to_str().with_context(|| {
        format!(
            "Failed to convert input path to string: {:?}",
            cli.input_path
        )
    })?;

    let tmp_dir_path = temp_dir();
    // Create a new project under tmp_dir using the current std time now
    let project_dir_path = tmp_dir_path.join(format!(
        "dragonfly-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    ));
    debug!("Creating project directory: {:?}", project_dir_path);
    std::fs::create_dir_all(&project_dir_path)?;

    let frames = 360;
    stdout.write_line(&format!(
        "[1/2] Extracting {} frames from {}",
        frames, input_path_str
    ))?;
    let pb = ProgressBar::new(frames);
    // Calculate the output image resolution based on the input and output FOVs and initial input resolution
    let h_ratio = cli.h_fov / cli.ih_fov;
    let v_ratio = cli.v_fov / cli.iv_fov;
    let output_width = (ffprobe_stream_output.width as f32 * h_ratio) as i32;
    let output_height = (ffprobe_stream_output.height as f32 * v_ratio) as i32;
    // Extract frames
    for frame in 0..frames {
        // Want to exclude +180.0 from the yaw calculation to avoid having duplicate starting and ending frames
        //let yaw = -180.0 + 360.0 * (frame as f32 / (frames - 1) as f32);
        let yaw = -180.0 + 360.0 * (frame as f32 / frames as f32);
        let pitch = 0.0;
        let roll = 0.0;
        let output_path = project_dir_path.join(format!("frame_{:08}.jpg", frame));
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
                "v360=e:flat:yaw={}:pitch={}:roll={}:h_fov={}:v_fov={}:interp={}:w={}:h={}",
                yaw,
                pitch,
                roll,
                cli.h_fov,
                cli.v_fov,
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
        let mut ffmpeg_child = ffmpeg_cmd.stdout(Stdio::piped()).spawn()?;
        ffmpeg_child.wait()?;
        pb.inc(1);
    }
    pb.finish_and_clear();

    let output = "output.mp4";
    stdout.write_line(&format!("[2/2] Encoding {} frames to {}", frames, output))?;
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
    // Encode output
    let frame_path_template = project_dir_path.join("frame_%08d.jpg");
    let frame_path_template_str = frame_path_template.to_str().with_context(|| {
        format!(
            "Failed to convert output path to string: {:?}",
            project_dir_path.join("frame_%08d.jpg")
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
        "1",
        "-i",
        frame_path_template_str,
        "-c:v",
        "libx264",
        "-r",
        "30",
        "-y",
        output,
    ]);
    debug!("Spawning command: {:?}", &ffmpeg_cmd);
    let mut ffmpeg_child = ffmpeg_cmd.stdout(Stdio::piped()).spawn()?;
    ffmpeg_child.wait()?;
    pb.finish_and_clear();
    std::process::exit(exitcode::OK);
}
