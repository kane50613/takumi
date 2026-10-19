# Encode a CSS keyframe animation with ffmpeg

Render a CSS keyframe animation frame by frame with `takumi-js`, then encode the frames as an H.265 MP4 with ffmpeg.

[![Animation preview](output/thumbnail.webp)](output/animation.mp4)

## Run

Install [FFmpeg](https://ffmpeg.org) with `libx265` support. Build the [image example packages](../README.md#image-examples). Then render the video:

```bash
cd example/ffmpeg-keyframe-animation
bun start
```

The script writes `output/thumbnail.webp`, a still at 2.5 seconds, and `output/animation.mp4`, a 4-second clip at 30 fps.

## How it works

Each frame is one `render()` call with `format: "raw"`, which returns RGBA pixels. Frame `i` sets the animation clock to:

```ts
const timeMs = (i / fps) * 1000;
```

Animation properties such as `animation-duration` and `animation-delay` resolve against that time. The script writes each frame to ffmpeg's stdin, and ffmpeg encodes the stream with `libx265`.

## Credits

Background photo by Martin Martz on Unsplash.
