# core/math-kernel

Julia math kernel (**SFIMathKernel**).

Phase 2 primitives: threshold masks, bright-pixel stats, connected-component count.

```bash
julia --project=core/math-kernel -e 'using SFIMathKernel; println(bright_pixel_count(UInt8[1,2,200], 128))'
```

Typically consumed by out-of-process plugins (e.g. `plugins/vision-2d`).

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Layer: Core
