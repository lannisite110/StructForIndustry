# plugins/vision-2d

2D vision utilities — shares the **same defect pipeline** as `defect-detect` (`SFIDefectDetect.process_defect_task`).

Used by `domains/industrial-inspection` (AOI line) when `vision.plugin: vision-2d` in profile.

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

Part of StructForIndustry · Layer: Tech plugin
