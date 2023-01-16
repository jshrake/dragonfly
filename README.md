# dragonfly

Software to extract frames from equirectangular (360) images and video.

Convert this

![example](https://user-images.githubusercontent.com/3046816/212596686-ba1b7ed0-5143-40b6-a2a5-0456a89cf0c3.jpg)

into this

[example.webm](https://user-images.githubusercontent.com/3046816/212596188-88efb730-8ecb-4b99-b133-b9fca26f176a.webm)


## Getting Started

You need to install <https://ffmpeg.org/> and ensure that the `ffmpeg` and `ffprobe` binaries are on your `$PATH`.

### MacOS

```console
brew install ffmpeg
```

## Usage

Here are some examples of things you can do with this software:

- Create a 5 second 30 FPS video from a 360 image

```bash
RUST_LOG=debug cargo run -- extract examples/example.jpg --frame-count 300 --j 8    
RUST_LOG=debug cargo run -- encode --length 5 --fps 30 --scale 0.125
```

## Resources

### Projections

- <https://blog.nitishmutha.com/equirectangular/360degree/2017/06/12/How-to-project-Equirectangular-image-to-rectilinear-view.html>
- <https://github.com/NitishMutha/equirectangular-toolbox/blob/master/nfov.py>
- <https://mathworld.wolfram.com/GnomonicProjection.html>

### ffmpeg

- <https://ffmpeg.org/ffmpeg-filters.html#v360>
- <https://ffmpeg.org/ffmpeg-filters.html#zoompan>
- <https://trac.ffmpeg.org/wiki/Encode/H.264>
- <https://superuser.com/questions/1112617/ffmpeg-smooth-zoompan-with-no-jiggle/1112680#1112680>
- <https://superuser.com/questions/1127615/ffmpeg-zoompan-filter-examples>
- <https://trac.ffmpeg.org/wiki/ChangingFrameRate>
- <https://blog.programster.org/ffmpeg-create-smooth-videos-with-frame-interpolation>
- <https://stackoverflow.com/questions/22547253/how-do-i-reduce-frames-with-blending-in-ffmpeg>
- <https://superuser.com/questions/564402/explanation-of-x264-tune>
- List of useful example commands <https://github.com/Fescron/ubuntu/blob/master/2-timelapse-ffmpeg.md#2---timelapse-to-video-with-ffmpeg>
- Another list <https://gist.github.com/jkalucki/c81f8fe17599a8c9cd51b565d7dc27eb>
