# AI inference plugin (Phase 4)

Inference sidecar for `infer.*` task types. Same plugin wire v1 as `defect-detect`.
Three scoring paths, picked by env (anomaly > onnx > mock):

1. **OK-only anomaly** (`SFI_ANOMALY_MODEL`) — PatchCore/EfficientAD-lite. Calibrate
   from defect-free frames only; score = per-cell nearest-neighbour distance to the
   OK memory bank, image score = worst cell. Affine-illumination invariant.
   Pluggable feature extractor (calibrate → bank → NN is identical either way):
   - `handcrafted` — `[mean, std, gradient]` per cell.
   - `onnx[:model.onnx]` — CNN feature map from an ONNX model via `ort`
     (`--features onnx`); without the runtime it falls back to a deterministic
     filter-bank emulation, so the pipeline still runs in CI. Generate a sample
     extractor with `python tools/scripts/gen-feature-onnx.py`.
2. **ONNX** (`SFI_ONNX_MODEL`) — `ort` backend with `--features onnx`, else a
   reference stub matching `tools/fixtures/models/tiny-defect.onnx`.
3. **Mock** — deterministic fallback.

## Run

```bash
cargo run --manifest-path plugins/ai-infer/Cargo.toml
```

Env:

| Variable | Default |
|----------|---------|
| `SFI_ANOMALY_MODEL` | unset (OK-only model JSON; takes priority) |
| `SFI_ONNX_MODEL` | unset (ONNX model path) |
| `SFI_INFER_SOCKET` | `$XDG_RUNTIME_DIR/sfi-infer.sock` |
| `SFI_VISION_PLUGIN_SOCKET` | fallback socket path |

## `sfi-anomaly` tool

Calibrate OK-only models and produce the three reports — no camera or training stack needed.

```bash
# Calibrate from N synthetic OK frames (replace with bench-rig frames later)
# --extractor handcrafted | onnx[:model.onnx]   (default: onnx reference)
cargo run --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- \
  calibrate --ok 20 --out tools/fixtures/models/anomaly-ok.json --extractor onnx

# Score one OK/defect frame
cargo run --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- \
  score --model tools/fixtures/models/anomaly-ok.json --defect

# Dump a raw Gray8 frame for replay through line-publisher (SFI_LINE_FRAME_FILE)
cargo run --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- \
  dump --kind ng --out /tmp/ng.gray8

# Reports -> docs/reports/ (changeover / latency / illumination / error-rates)
bash tools/scripts/anomaly-reports.sh
```

## E2E

```bash
bash tools/scripts/anomaly-infer-e2e.sh   # defect → NG, OK → OK through the bus
bash tools/scripts/onnx-infer-e2e.sh      # onnx reference path → NG
```

## Task types

- `infer.onnx` / `infer.anomaly` / `infer.mock`

Part of [sfi-platform](https://github.com/lannisite110/StructForIndustry).
