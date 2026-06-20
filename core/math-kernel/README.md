# core/math-kernel

Julia math kernel (**SFIMathKernel**) — zero third-party image libs.

Phase A industrial primitives:

| Category | Functions |
|----------|-----------|
| Stats | `gray_stats`, `gray_histogram` |
| Threshold | `gray_threshold`, `otsu_threshold`, `adaptive_threshold`, `threshold_mask` |
| Preprocess | `gaussian_blur_3x3`, `median_filter_3x3`, `apply_preproc` |
| Morphology | `morph_erode/dilate/open/close_3x3`, `apply_morph` |
| Blobs | `connected_components_labels`, `blob_stats_from_labels`, `filter_blobs`, `largest_blob` |
| Measure | `edge_caliper_horizontal/vertical`, `measure_line_width_horizontal`, `measure_circle_diameter_horizontal`, `fit_line`, `fit_circle` |
| Template | `ncc_score_at`, `ncc_match`, `extract_template` |
| Legacy | `bright_pixel_count`, `connected_components_count` |

```bash
julia --project=core/math-kernel -e 'using Pkg; Pkg.test()'
```

Consumed by `defect-detect`, `vision-2d`, and domain plugins.

Part of [sfi-platform](https://github.com/lannisite110/StructForIndustry) · Layer: Core
