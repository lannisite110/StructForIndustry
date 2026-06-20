# AI inference plugin (Phase 4 scaffold)

Mock ONNX sidecar for `infer.*` task types. Same plugin wire v1 as `defect-detect`.

## Run

```bash
cargo run --manifest-path plugins/ai-infer/Cargo.toml
```

Env:

| Variable | Default |
|----------|---------|
| `SFI_INFER_SOCKET` | `$XDG_RUNTIME_DIR/sfi-infer.sock` |
| `SFI_VISION_PLUGIN_SOCKET` | fallback socket path |

## Task types

- `infer.onnx` — mock GPU inference result
- `infer.mock` — alias behaviour

## Next steps (Phase 4)

- Wire `ort` / ONNX Runtime backend
- GPU memory quota in `plugin-host`
- Profile switch: traditional CV vs DL (`vision.plugin` vs `infer.plugin`)

Part of [sfi-platform](https://github.com/lannisite110/StructForIndustry).
