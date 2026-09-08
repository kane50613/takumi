# FFplay Example

Render a moving clock as raw RGBA frames with Takumi and play them with `ffplay`.

## Prerequisites

- [Bun](https://bun.sh)
- [FFmpeg](https://ffmpeg.org) (specifically `ffplay`)

## Usage

```bash
bun install
bun src/index.ts
```

Press `Ctrl+C` to exit.

## How It Works

1. Renders a clock with a target frame rate of 60 fps
2. Outputs raw RGBA frames
3. Pipes frames to `ffplay` for real-time playback
