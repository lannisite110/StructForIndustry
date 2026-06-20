#!/usr/bin/env python3
"""Generate tiny-feature.onnx — a small CNN feature extractor for the OK-only
anomaly path. Input [1,1,H,W] (standardized gray), output a [1,C,Hf,Wf] feature
map that `sfi-anomaly --extractor onnx:tiny-feature.onnx` pools per grid cell.

Fixed (not learned) Conv filters mirror the Rust filter-bank reference
(identity / Sobel-x / Sobel-y / Laplacian / blur), so behaviour matches with or
without `ort`. Swap in a trained backbone (ResNet/WideResNet layer) later — the
calibrate -> bank -> NN pipeline is unchanged.
"""
from pathlib import Path

try:
    import numpy as np
    import onnx
    from onnx import TensorProto, helper, numpy_helper
except ImportError as e:
    raise SystemExit("pip install onnx numpy") from e

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tools" / "fixtures" / "models" / "tiny-feature.onnx"
OUT.parent.mkdir(parents=True, exist_ok=True)

C = 5  # feature channels
kernels = np.array(
    [
        [[0, 0, 0], [0, 1, 0], [0, 0, 0]],      # identity
        [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]],   # Sobel-x
        [[-1, -2, -1], [0, 0, 0], [1, 2, 1]],   # Sobel-y
        [[0, 1, 0], [1, -4, 1], [0, 1, 0]],     # Laplacian
        [[1, 1, 1], [1, 1, 1], [1, 1, 1]],      # blur (box)
    ],
    dtype=np.float32,
).reshape(C, 1, 3, 3)
kernels[4] /= 9.0  # normalize box blur

W = numpy_helper.from_array(kernels, name="W")

inp = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 1, None, None])
feat = helper.make_tensor_value_info("features", TensorProto.FLOAT, [1, C, None, None])

conv = helper.make_node(
    "Conv", ["input", "W"], ["features"], name="feat_conv", pads=[1, 1, 1, 1]
)

graph = helper.make_graph([conv], "tiny_feature", [inp], [feat], initializer=[W])
model = helper.make_model(
    graph,
    opset_imports=[helper.make_opsetid("", 13)],
    producer_name="sfi-tools",
)
onnx.checker.check_model(model)
onnx.save(model, OUT)
print(f"Wrote {OUT} ({OUT.stat().st_size} bytes), {C} feature channels")
