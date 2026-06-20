# plugins/vision-2d

2D vision utilities — detection, measurement, SPC hooks.

Used heavily by `domains/industrial-inspection` and reusable by robotics / medical imaging.

## Run (Julia sidecar)

```bash
# Terminal 1 — bus with scheduler
SFI_SCHEDULER=1 cargo run -p sfi-core-bus --bin sfi-bus

# Terminal 2 — Julia plugin (reads Gray8 from shm)
julia --project=plugins/vision-2d plugins/vision-2d/server.jl
```

Default socket: `$XDG_RUNTIME_DIR/sfi-plugin-vision.sock` (override with `SFI_VISION_PLUGIN_SOCKET` / `SFI_VISION_SOCKET`).

Wire format: [`core/contracts/plugin_wire.md`](../core/contracts/plugin_wire.md).

## CI mock

Rust mock sidecar (no Julia required): `sfi_plugin_host::run_mock_vision_sidecar` — used by `core/core-bus/tests/vision_e2e.rs`.

Part of [sfi-platform](https://github.com/StructForIndustry/sfi-platform) · Layer: Tech plugin
