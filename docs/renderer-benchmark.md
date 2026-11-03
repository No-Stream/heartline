# Renderer benchmark

This benchmark compares the custom instanced renderer with PixiJS 8.22's
`ParticleContainer` on the same synthetic isometric scene. It exercises WebGPU
and WebGL2 in headless Chrome and checks a few rendered pixels. It is intended
to inform the renderer choice; it does not measure the finished game's art,
input, full simulation, or complete frame pipeline.

## What the benchmark measures

The scene contains 65,536 terrain sprites, 10,000 animated people, 2,048
overlapping track segments, three tunnel sprites, and 12 marker sprites. The
canvas is 1920 × 1080 pixels at resolution 1. Both implementations use the same
generated 16-pixel atlas and sprite data from [the scene generator]. The custom
renderer submits sprites as instanced quads. The PixiJS path creates particles
from the same data, sorts them by sprite depth into painter order, updates its
`ParticleContainer`, and renders the application. All five Pixi particle
properties are configured as dynamic to cover per-frame position, shape,
rotation, atlas, and color updates. PixiJS documents its particle API and
per-frame dynamic properties in the [ParticleContainer] documentation; the
[rendering overview] describes its renderer backends.

For each configuration, the page runs for at least 10 seconds. The [page] records
the mean and 95th-percentile intervals between `requestAnimationFrame` callbacks,
the frame count, and elapsed time. This is a browser frame-cadence measurement,
not an isolated CPU render duration or a GPU timestamp measurement. Renderer
and scene setup happen before timing starts, and there is no separate warm-up
period. Browser scheduling and queued GPU work can affect the results.

After each measured run, the harness samples six known canvas pixels: terrain,
person, track, tunnel, track-over-track, and a transparent edge. These markers
catch selected rendering and ordering mistakes; they do not establish that the
full images are identical or that the scene is visually ready for release.

## Depth and correctness limits

The custom WebGL2 and WebGPU paths assign one depth value to each sprite quad.
All vertices of a quad use the same value, and the depth buffer tests the
resulting fragments against that value. Transparent texels are discarded. This
can place one sprite in front of another, but it does not describe changing
surface depth across a sprite or resolve intersections between arbitrary
geometry. It is not a general per-pixel geometric solution for isometric
occlusion.

The PixiJS path sorts whole particles by one depth value on the CPU and draws
them in that order. This is painter ordering at sprite granularity, not
per-pixel geometric visibility either. Pixi provides depth attachments and a
main-view depth option, but its standard particle path here does not consume
our per-sprite depth; using the buffer would require custom rendering pipeline
work. Both paths can test this benchmark's
chosen sprite overlaps; neither result should be read as proof that bridges,
roofs, tall structures, or intersecting shapes will have correct game-wide
occlusion. The WebGPU specification describes depth comparison on fragments,
while the benchmark shaders provide each quad with a single depth value.

## Run it

Install the JavaScript dependencies once, then run the automated benchmark:

```sh
make install
make bench-render
```

The command starts a temporary local Vite server and launches headless Chrome
through Playwright. It prints the detected GPU information and WebGL2 device,
then reports results for custom and PixiJS with WebGPU and WebGL2. The [runner]
checks pixel markers for each configuration. A missing
WebGPU adapter is reported as unavailable; other configuration or marker
failures fail the command. The script also checks that automatic selection
falls back to WebGL2 when it hides `navigator.gpu`.

To inspect or run a configuration interactively, start the regular Vite server
and open the benchmark page:

```sh
make serve
```

Open <http://127.0.0.1:5173/bench/>. The page lets you choose `custom` or
`pixi`, and `auto`, `webgpu`, or `webgl2`. Automatic selection chooses WebGPU
when the browser returns an adapter and otherwise chooses WebGL2. WebGPU
availability depends on the browser, operating system, driver, and adapter; the
[GPUWeb implementation status] tracks those qualifications. Check the device
reported by the run before comparing results across machines.

For a software-rendered run using Chrome's SwiftShader paths, use:

```sh
BENCH_SWIFTSHADER=1 make bench-render
```

This adds Chrome's Vulkan and ANGLE SwiftShader selection flags. Chromium
documents SwiftShader as its software GL rasterizer. Treat these timings as
relative results for the same software setup only: they do not predict absolute
frame rates or renderer rankings on a hardware GPU. Keep software and hardware
results separate, and record the reported device information with each result.

## Results

On November 2, 2026, Chrome 146.0.7680.177 ran in the WSL2 sandbox with
hardware-seeking flags. The operating system blocked NVML GPU access and Chrome
reported SwiftShader. The default WebGPU configuration produced a transparent
canvas, so it did not pass the pixel check. With explicit SwiftShader Vulkan and
ANGLE flags, all four configurations completed and all six markers passed.
The final run used an isolated Vite cache. These numbers are software-only, use few frames in the slowest cases, and cannot
establish RTX 5090 performance or predict the hardware ranking.

| Renderer | Backend | Adapter | Mean interval (ms) | p95 interval (ms) | Frames | Duration (ms) |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| Custom | WebGPU | Google SwiftShader | 1331.200 | 1983.3 | 8 | 10649.6 |
| Custom | WebGL2 | ANGLE SwiftShader | 1077.038 | 1683.3 | 10 | 10770.378 |
| PixiJS | WebGPU | Google SwiftShader | 123.657 | 150.0 | 81 | 10016.2 |
| PixiJS | WebGL2 | ANGLE SwiftShader | 44.319 | 50.1 | 226 | 10016.2 |

Both automatic selections chose WebGL2 with WebGPU disabled and passed all six
markers in this software run. Run the interactive page in normal Chrome to
measure the actual GPU; the on-page result includes the adapter for WebGPU,
and its actual renderer adapter. Chrome's `chrome://gpu` provides additional device details.

The fallback check is a correctness smoke check: it confirms that `auto`
selects WebGL2 when WebGPU is unavailable. The runner logs whether the check
passed, but it does not include fallback timings in its result table.

## Renderer choice

I recommend PixiJS as the initial approach for the game: it owns the backend
plumbing and its particle API handled the scene and selected depth checks on
both software backends. It was also faster in this particular software run,
though that result cannot settle hardware performance. The particle API is
experimental, so pinning the version and rechecking upgrades matters. The game
still needs representative hardware measurements and a design for occlusion of
geometry that cannot be represented by a single depth per sprite. If those
measurements show PixiJS misses the frame budget, revisit the custom renderer.

As of November 2, 2026, the [GPUWeb implementation status] lists Firefox on
Linux as enabled by default in Nightly, rather than stable Firefox. It lists
Chrome Linux support for Intel Gen12+ from Chrome 144 and NVIDIA with driver
535.183.01 or newer on Wayland from Chrome 147; other Linux configurations
may require flags. WebGL2 remains required. PixiJS's
[rendering overview] recommends WebGL for production and calls WebGPU
experimental. The benchmark's automatic fallback is a prototype; the existing
game renderer was deliberately not changed.

## References

- [GPUWeb implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status)
- [PixiJS 8.22 rendering overview](https://pixijs.download/v8.22.0/docs/rendering.html)
- [PixiJS 8.22 ParticleContainer](https://pixijs.download/v8.22.0/docs/scene.ParticleContainer.html)
- [PixiJS 8.22 ParticleProperties](https://pixijs.download/v8.22.0/docs/scene.ParticleProperties.html)
- [WebGPU depth/stencil state](https://gpuweb.github.io/gpuweb/#depth-stencil-state)
- [Chromium GPU compositing and SwiftShader](https://new.chromium.org/developers/design-documents/gpu-accelerated-compositing-in-chrome/)

[scene generator]: ../web/bench/scene.ts
[page]: ../web/bench/main.ts
[runner]: ../web/bench/run.mjs
[ParticleContainer]: https://pixijs.download/v8.22.0/docs/scene.ParticleContainer.html
[rendering overview]: https://pixijs.download/v8.22.0/docs/rendering.html

