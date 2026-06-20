# plugins

Technical plugins for the industrial-inspection platform.

| Plugin | Path | Language | Description |
|--------|------|----------|-------------|
| AI inference | [ai-infer](ai-infer/) | Rust | ONNX (`ort`) + OK-only anomaly model serving |
| 2D vision | [vision-2d](vision-2d/) | Rust / Julia | detection, measurement helpers |

Domain-specific business logic lives under `domains/*/plugins/`.

Part of [sfi-platform](https://github.com/lannisite110/StructForIndustry)
