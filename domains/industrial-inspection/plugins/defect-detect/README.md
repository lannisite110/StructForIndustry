# defect-detect — industrial surface defect plugin

Domain business plugin for `industrial-inspection`. Implements plugin wire v1 sidecar using `SFIDefectDetect` pipeline.

## Run

```bash
# with bus scheduler pointing at same socket
julia --project=domains/industrial-inspection/plugins/defect-detect server.jl
```

Env: `SFI_VISION_SOCKET`, `XDG_RUNTIME_DIR`.

## CI mock (Rust)

```bash
cargo run -p sfi-plugin-host --bin sfi-mock-defect-detect
```

## Metrics emitted

| Name | Description |
|------|-------------|
| `gray_mean` | Frame mean gray level |
| `bright_pixels` | Pixels ≥ threshold |
| `defect_components` | Connected components count |

SPC engine on bus rolls `gray_mean` and `ng_rate` into topic `spc.metrics`.
