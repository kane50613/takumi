# Play live Takumi frames with ffplay

Render a bouncing clock as raw RGBA frames and play them in real time with `ffplay`.

## Run

Install an [FFmpeg](https://ffmpeg.org) build that includes `ffplay`. Some builds leave it out. Build the [image example packages](../README.md#image-examples). Then start the clock:

```bash
cd example/ffplay
bun start
```

Press `Ctrl+C` to stop.

## How it works

The script keeps one `Renderer` from `takumi-js/node` and renders each 960 × 540 frame with `format: "raw"`. It pipes the pixels to `ffplay` at a target of 60 fps. The next frame renders while the current one is written. The clock position comes from wall-clock time, so a late frame does not change its path.
