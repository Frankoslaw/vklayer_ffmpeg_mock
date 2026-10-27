# Vulkan H.264 Mock Layer

`VK_LAYER_FFMPEG_mock` is an explicit Vulkan layer for applications that need
to discover Vulkan Video H.264 encode support. It also logs selected Vulkan
calls through Rust's `log` facade.

## What it does

When the ICD does not advertise Vulkan Video encode support, the layer reports
the video queue and H.264 device extensions Moonshine expects, and supplies
synthetic H.264 capability limits. Synthetic extensions are removed before
`vkCreateDevice` is forwarded, so the driver only receives extensions it
advertises. Other Vulkan calls, including video session and encode commands,
are forwarded to the next layer or ICD.

This layer does not encode frames itself and does not route encoding through
FFmpeg or VA-API. Its effect depends on the Vulkan driver behind it.

## Motivation and Moonshine behavior

Moonshine uses Pixelforge's Vulkan Video encoder for H.264. Its health check
uses Vulkan capability discovery to decide whether to offer that codec. On
some GPUs and driver setups, the driver may expose enough of the Vulkan Video
implementation to run encoding while not advertising the required H.264
support during discovery. The layer makes those capabilities visible so the
application can attempt the native Vulkan Video path; the actual video session
and encode commands still go to the driver. A successful stream therefore
depends on the driver being able to execute them.

Moonshine's `--no-health-check` option does not select another encoder or skip
all capability probing. It lets the app continue after failed checks, but the
probe can still report no supported codecs. The app may start without H.264,
then fail when its Vulkan encoder is initialized for a stream. This layer was
created to investigate and work around that capability-discovery gap, and to
trace the Vulkan calls involved. It does not provide a software encoder or
make unsupported hardware encode video.

## Build

From this repository:

```sh
cargo build --release
```

The library is written to `target/release/libVkLayer_ffmpeg_mock.so`; the
loader manifest is `VkLayer_ffmpeg_mock.json` in the repository root.

## Use with another program

Run the program with the repository as the Vulkan layer search path and enable
the layer by name:

```sh
export VK_LAYER_PATH="$PWD${VK_LAYER_PATH:+:$VK_LAYER_PATH}"
export VK_INSTANCE_LAYERS="VK_LAYER_FFMPEG_mock${VK_INSTANCE_LAYERS:+:$VK_INSTANCE_LAYERS}"
export RUST_LOG=info
your-vulkan-program [arguments]
```

Run `your-vulkan-program` from this repository, or set `VK_LAYER_PATH` to its
absolute path. The manifest's library path is relative to the manifest file.

For detailed `vkGetInstanceProcAddr` and `vkGetDeviceProcAddr` routing logs,
set `RUST_LOG=debug`. `info` logs intercepted calls and results; `warn` is the
default.

## Test with Moonshine

With Moonshine's source checkout at `../moonshine`, run:

```sh
./examples/test-moonshine.sh
```

Pass a different config file as the first argument when needed:

```sh
./examples/test-moonshine.sh /path/to/config.toml
```

The script builds this layer, sets `VK_LAYER_PATH` and
`VK_INSTANCE_LAYERS=VK_LAYER_FFMPEG_mock`, then runs Moonshine from source with
the config file. To enable layer traces for the run:

```sh
RUST_LOG=info ./examples/test-moonshine.sh
RUST_LOG=debug ./examples/test-moonshine.sh
```
