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
| `bright_pixels` | Pixels ≥ threshold used |
| `defect_components` | Blobs after morphology + area/aspect filter |
| `threshold_used` | Fixed / Otsu / adaptive effective threshold (DN) |

Profile `vision.algorithm` controls preproc (`none|gaussian_3x3|median_3x3`), `thresholdMode` (`fixed|otsu|adaptive`), `morph` (`none|open_3x3|close_3x3`), and `blob` filters.

SPC engine on bus rolls `gray_mean` and `ng_rate` into topic `spc.metrics`.
