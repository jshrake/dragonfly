[[Go back]](README.md)

<br/>

## Table of contents

- [Table of contents](#table-of-contents)
- [2 - Timelapse to video with ffmpeg](#2---timelapse-to-video-with-ffmpeg)
  - [2.1 - General commands](#21---general-commands)
  - [2.2 - Options explained](#22---options-explained)
  - [2.3 - Other useful commands](#23---other-useful-commands)

<br/>

------

<br/>

## 2 - Timelapse to video with ffmpeg

### 2.1 - General commands

No crop (add black bars where needed) - No upscaling

```bash
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale='min(3840,iw)':min'(2160,ih)':force_original_aspect_ratio=decrease,pad=3840:2160:(ow-iw)/2:(oh-ih)/2" 4k-30fps.mp4
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale='min(2560,iw)':min'(1440,ih)':force_original_aspect_ratio=decrease,pad=2560:1440:(ow-iw)/2:(oh-ih)/2" 1440p-30fps.mp4
```

<br/>

Cropping from center

```bash
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=3840:2160:force_original_aspect_ratio=increase,crop=3840:2160" 4k-30fps-crop.mp4
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=2560:1440:force_original_aspect_ratio=increase,crop=2560:1440" 1440p-30fps-crop.mp4
```

<br/>

Cropping from center (Instagram)

```bash
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=1080:1080:force_original_aspect_ratio=increase,crop=1080:1080" 0-insta-1x1-30fps-crop.mp4
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=1080:1350:force_original_aspect_ratio=increase,crop=1080:1350" 0-insta-4x5-30fps-crop.mp4
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=1080:566:force_original_aspect_ratio=increase,crop=1080:566" 0-insta-5x4-30fps-crop.mp4
```

If the images were taken in portrait orientation, use the following command to make sure the EXIF data is reflected in the image size.

```bash
mogrify -auto-orient *.JPG
```

<br/>

Cropping from bottom

```bash
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=3840:2160:force_original_aspect_ratio=increase,crop=3840:2160:0:oh" 4k-30fps-crop-bottom.mp4
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=2560:1440:force_original_aspect_ratio=increase,crop=2560:1440:0:oh" 1440p-30fps-crop-bottom.mp4
```

<br/>

Cropping from top

```bash
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=3840:2160:force_original_aspect_ratio=increase,crop=3840:2160:0:0" 4k-30fps-crop-top.mp4
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "scale=2560:1440:force_original_aspect_ratio=increase,crop=2560:1440:0:0" 1440p-30fps-crop-top.mp4
```

<br/>

Adding a watermark

```bash
ffmpeg -i 0-insta-4x5-30fps-crop.mp4 -r 30 -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "drawtext=text='@brecht.ve':x=10:y=H-th-13:fontfile='/home/brecht/.fonts/BebasNeue Book.otf':fontsize=35:fontcolor=white@0.65" 0-insta-4x5-30fps-crop-watermark.mp4
```

<br/>

Other encoding profile, more contrast (may not work on every player)

```bash
ffmpeg -r 30 -pattern_type glob -i '*.JPG' -vcodec libx264 -profile:v high422 -crf 20 -tune film -vf "scale='min(3840,iw)':min'(2160,ih)':force_original_aspect_ratio=decrease,pad=3840:2160:(ow-iw)/2:(oh-ih)/2" 4k-30fps-high422.mp4
```

<br/>

Select all images in all subdirectories (recursively), don't crop/enlarge

```bash
ffmpeg -r 30 -pattern_type glob -i '**/*.jpg' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film timelapse-30fps.mp4
```

<br/>

Select all images in (sub)directories with specific names (dates) and in the correct order, don't crop/enlarge, save to the desktop (for when the source pictures are in a remote location)

```bash
ffmpeg -r 45 -pattern_type glob -i '2020-04-{06,07,08,09,10,11}/{07,08,09,10,11,12,13,14,15,16,17,18,19}/*.jpg' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film ~/Desktop/timelapse.mp4
```

<br/>

Select all images in (sub)directories with specific names (dates) and in the correct order, don't crop/enlarge, increase framerate using `minterpolate` filter from 30 to 60fps for smoother looking timelapse

```bash
ffmpeg -r 45 -pattern_type glob -i '2020-04-{02,03,04}/{07,08,09}/*.jpg' -vcodec libx264 -crf 20 -pix_fmt yuv420p -tune film -vf "minterpolate=fps=60" timelapse-blurred.mp4
```

<br/>

### 2.2 - Options explained

| Command | Meaning |
|---------|---------|
| `-r 30` | Output frame rate |
| `-pattern_type glob -i '*.JPG'` | All JPG files in the current directory |
| `'*/*.JPG'` | All JPG files from all directories, one level down from the current directory |
| `'**/*.JPG'` |  All JPG files from all directories, all levels down from the current directory (recursively) |
| `-vcodec libx264` | H.264 encoding (mp4) |
| `-crf 20` | Constant Rate Factor (lower = better, anything below 18 might not be visually better, 23 default) 20 would be good since YouTube re-encodes the video again |
| `-pix_fmt yuv420p` | Enable YUV planar color space with 4:2:0 chroma subsampling for H.264 video (so the output file works in QuickTime and most other players) |
| `-tune film` | Intended for high-bitrate/high-quality movie content. Lower deblocking is used here. |
| `-vf "minterpolate=fps=60"` | Use `minterpolate` videofilter to interpolate images and make a smoother video. **Slow because only uses single CPU core!** |

<br/>

**Other** `-tune` **options:**
- `-tune grain` This should be used for material that is already grainy. Here, the grain won't be filtered out as much.
- `-tune fastdecode` Disables CABAC and the in-loop deblocking filter to allow for faster decoding on devices with lower computational power.
- `-tune zerolatency` Optimization for fast encoding and low latency streaming.

<br/>

**Unused options:**
- `-preset veryfast` Encoding speed. A slower preset provides better compression (quality per file size) but is slower. Use the slowest that you have patience for.
  - Possibilities: `ultrafast`, `superfast`, `veryfast`, `faster`, `fast`, `medium` (default), `slow`, `slower`, `veryslow`.

<br/>

### 2.3 - Other useful commands

Convert JPGs to 1920x1080, centered

```bash
convert input.jpg -resize '1920x1080^' -gravity center -crop '1920x1080+0+0' output.jpg
```

<br/>

Renaming

```bash
mkdir renamed; num=0; for f in $(ls -tr); do cp -p "$f" renamed/IMG_$(printf "%04d" $num).JPG; printf "\n\r$num"; num=$((num+1)); done
```
