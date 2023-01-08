use log::debug;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use strum::{Display, EnumString};
use thiserror::Error;

static FFMPEG_BINARY_PATH_DEFAULT: &str = if cfg!(target_os = "windows") {
    "ffmpeg.exe"
} else {
    "ffmpeg"
};

static FFPROBE_BINARY_PATH_DEFAULT: &str = if cfg!(target_os = "windows") {
    "ffprobe.exe"
} else {
    "ffprobe"
};

lazy_static::lazy_static! {
    pub static ref FFMPEG_BINARY_PATH: OsString = std::env::var("FFMPEG_BINARY_PATH")
        .unwrap_or_else(|_| FFMPEG_BINARY_PATH_DEFAULT.to_string()).into();
    pub static ref FFPROBE_BINARY_PATH: OsString = std::env::var("FFPROBE_BINARY_PATH")
        .unwrap_or_else(|_| FFPROBE_BINARY_PATH_DEFAULT.to_string()).into();
}

#[derive(Debug, Error)]
pub enum DragonflyError {
    #[error("Source input contains no streams")]
    SourceContainsNoStream,
    #[error("Error while running ffprobe: {0}")]
    Command(#[from] std::io::Error),
    #[error("Error serializing JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Error converting path to str: {0}")]
    InvalidPathString(PathBuf),
    #[error("Error extracting images with ffmpeg")]
    FfmpegExtractFailed,
    #[error("Unknown error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, DragonflyError>;

#[derive(Debug)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct ExtractFramesDescriptor {
    #[cfg_attr(
        feature = "clap",
        arg(help = "Number of frames to extract", long, default_value = "360")
    )]
    pub frame_count: usize,
    #[cfg_attr(
        feature = "clap",
        arg(
            help = "The horizontal field of view in degrees of the input image",
            long,
            default_value = "360.0"
        )
    )]
    pub ih_fov: f32,
    #[cfg_attr(
        feature = "clap",
        arg(
            help = "The vertical field of view in degrees of the input image",
            long,
            default_value = "180.0"
        )
    )]
    pub iv_fov: f32,
    #[cfg_attr(
        feature = "clap",
        arg(
            help = "The horizontal field of view in degrees of the extracted output images",
            long,
            default_value = "60.0"
        )
    )]
    pub h_fov: f32,
    #[cfg_attr(
        feature = "clap",
        arg(
            help = "The vertical field of view in degrees of the extracted output images",
            long,
            default_value = "45.0"
        )
    )]
    pub v_fov: f32,
    #[cfg_attr(
        feature = "clap",
        arg(help = "Number of CPU threads to use", long, default_value = "4")
    )]
    pub j: usize,
    #[cfg_attr(feature = "clap", arg(help = "Interpolation method to use", long, default_value_t = Interpolation::Linear))]
    pub interpolation: Interpolation,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct EncodeFramesDescriptor {
    #[cfg_attr(
        feature = "clap",
        arg(
            help = "The desired length in seconds of the video",
            long,
            default_value = "10"
        )
    )]
    pub length: f32,
    #[cfg_attr(
        feature = "clap",
        arg(help = "The FPS of the output video", long, default_value = "60")
    )]
    pub fps: f32,
    #[cfg_attr(
        feature = "clap",
        arg(help = "The scale of the output video", long, default_value = "1.0")
    )]
    pub scale: String,
}

#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[derive(Clone, Debug, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Interpolation {
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

fn ffprobe_info(input_path: &Path) -> Result<FfprobeOutput> {
    let input_path_str = input_path
        .to_str()
        .ok_or_else(|| DragonflyError::InvalidPathString(input_path.to_path_buf()))?;
    // Fetch the input pixel resolution
    let ffprobe_child = Command::new(FFPROBE_BINARY_PATH.as_os_str())
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
    Ok(ffprobe_output)
}

pub fn extract_frames(
    input_path: &Path,
    extraction_path: &Path,
    descriptor: &ExtractFramesDescriptor,
    progress_callback: Option<impl Fn(usize, usize)>,
) -> Result<()> {
    let input_path_str = input_path
        .to_str()
        .ok_or_else(|| DragonflyError::InvalidPathString(input_path.to_path_buf()))?;
    let ffprobe_output = ffprobe_info(input_path)?;
    let ffprobe_stream_output = ffprobe_output
        .streams
        .first()
        .ok_or(DragonflyError::SourceContainsNoStream)?;
    // Calculate the output image resolution based on the input and output FOVs and initial input resolution
    let ih_fov = descriptor.ih_fov;
    let iv_fov = descriptor.iv_fov;
    let oh_fov = descriptor.h_fov;
    let ov_fov = descriptor.v_fov;
    let h_ratio = oh_fov / ih_fov;
    let v_ratio = ov_fov / iv_fov;
    let output_width = (ffprobe_stream_output.width as f32 * h_ratio) as i32;
    let output_height = (ffprobe_stream_output.height as f32 * v_ratio) as i32;

    let mut tasks = Vec::with_capacity(descriptor.j);
    // Extract frames
    for frame in 0..descriptor.frame_count {
        //let yaw = -180.0 + 360.0 * (frame as f32 / (descriptor.frame_count - 1) as f32);
        // Want to exclude +180.0 from the yaw calculation to avoid having duplicate starting and ending frames
        let yaw = -180.0 + 360.0 * (frame as f32 / descriptor.frame_count as f32);
        let pitch = 0.0;
        let roll = 0.0;
        let output_path = extraction_path.join(format!("frame_{:08}.jpg", frame));
        let output_path_str = output_path
            .to_str()
            .ok_or_else(|| DragonflyError::InvalidPathString(output_path.clone()))?;
        let mut ffmpeg_cmd = Command::new(FFMPEG_BINARY_PATH.as_os_str());
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
                "v360=e:flat:yaw={}:pitch={}:roll={}:ih_fov={}:iv_fov={}:h_fov={}:v_fov={}:interp={}",
                yaw,
                pitch,
                roll,
                ih_fov,
                iv_fov,
                oh_fov,
                ov_fov,
                descriptor.interpolation,
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
                let status = task.wait()?;
                if !status.success() {
                    return Err(DragonflyError::FfmpegExtractFailed);
                }
            }
            tasks.clear();
        }
        if let Some(progress_callback) = progress_callback.as_ref() {
            progress_callback(frame, descriptor.frame_count);
        }
    }
    Ok(())
}

pub fn encode_frames(
    output_path: &Path,
    extraction_path: &Path,
    descriptor: &EncodeFramesDescriptor,
) -> Result<ExitStatus> {
    // Encode output
    let frame_path_template = extraction_path.join("frame_%08d.jpg");
    let frame_path_template_str = frame_path_template
        .to_str()
        .ok_or_else(|| DragonflyError::InvalidPathString(frame_path_template.clone()))?;
    let output_fps_string = descriptor.fps.to_string();
    // If the user passed in a scale factor, use that. Otherwise, use the scale string as-is
    let scale_filter_string = if let Ok(scale) = descriptor.scale.parse::<f32>() {
        format!("scale=iw*{scale}:ih*{scale}")
    } else {
        format!("scale={}", &descriptor.scale)
    };
    let mut ffmpeg_cmd = Command::new(FFMPEG_BINARY_PATH.as_os_str());
    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| DragonflyError::InvalidPathString(output_path.to_path_buf()))?;
    let total_frame_count = fs::read_dir(extraction_path)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().ok().map_or(false, |ft| ft.is_file()))
        .count();
    debug!("Total frame count {total_frame_count}");
    let input_frames_per_second = total_frame_count as f32 / descriptor.length;
    ffmpeg_cmd.args([
        // Quiet output
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostats",
        // Input FPS
        "-r",
        input_frames_per_second.to_string().as_str(),
        // Input directory path containing images
        "-i",
        frame_path_template_str,
        // h264
        "-c:v",
        "libx264",
        // preset
        "-preset",
        "slow",
        // crf
        "-crf",
        "18",
        // pixel format
        "-pix_fmt",
        "yuv420p",
        // TODO: configurable
        "-tune",
        "stillimage",
        // key frame the first and last frame
        "-g",
        &format!("{}", total_frame_count - 1),
        // Filters
        // - Frame interpolation/blending
        // - Scaling
        "-vf",
        &format!("{}", scale_filter_string.as_str(),),
        // output framerate
        // https://trac.ffmpeg.org/wiki/ChangingFrameRate
        "-r",
        output_fps_string.as_str(),
        // Output file path
        "-y",
        output_path_str,
    ]);
    debug!("Spawning command: {:?}", &ffmpeg_cmd);
    let mut ffmpeg_child = ffmpeg_cmd.stdout(Stdio::piped()).spawn()?;
    let status = ffmpeg_child.wait()?;

    Ok(status)
}
