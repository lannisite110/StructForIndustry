#!/usr/bin/env python3
"""Generate tiny-defect.onnx — GlobalAveragePool over [1,1,64,64] gray input."""
from pathlib import Path

try:
    import onnx
    from onnx import TensorProto, helper
except ImportError as e:
    raise SystemExit("pip install onnx") from e

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tools" / "fixtures" / "models" / "tiny-defect.onnx"
OUT.parent.mkdir(parents=True, exist_ok=True)

inp = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 1, 48, 64])
pooled = helper.make_tensor_value_info("pooled", TensorProto.FLOAT, [1, 1, 48, 64])
out = helper.make_tensor_value_info("output", TensorProto.FLOAT, [1, 1, 1, 1])

gap = helper.make_node("GlobalAveragePool", ["input"], ["pooled"], name="gap")
axes = helper.make_tensor("axes", TensorProto.INT64, [2], [2, 3])
squeeze = helper.make_node("Squeeze", ["pooled", "axes"], ["output"], name="squeeze")

graph = helper.make_graph(
    [gap, squeeze],
    "tiny_defect",
    [inp],
    [out],
    initializer=[axes],
)
model = helper.make_model(
    graph,
    opset_imports=[helper.make_opsetid("", 13)],
    producer_name="sfi-tools",
)
onnx.checker.check_model(model)
onnx.save(model, OUT)
print(f"Wrote {OUT} ({OUT.stat().st_size} bytes)")
