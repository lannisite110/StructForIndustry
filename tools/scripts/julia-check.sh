#!/usr/bin/env bash
# Julia syntax / load check for math-kernel and vision-2d
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "== math-kernel =="
julia --project=core/math-kernel -e '
using Pkg
Pkg.instantiate()
Pkg.test()
'

echo "== vision-2d (parse server_impl) =="
julia --project=plugins/vision-2d -e '
using Pkg
Pkg.instantiate()
include("plugins/vision-2d/server_impl.jl")
println("vision-2d server_impl OK")
'

echo "== defect-detect =="
julia --project=domains/industrial-inspection/plugins/defect-detect -e '
using Pkg
Pkg.instantiate()
include("domains/industrial-inspection/plugins/defect-detect/src/SFIDefectDetect.jl")
using SFIDefectDetect
c, b, g, bb, t = detect_surface_defects(fill(UInt8(200), 16), 4, 4; threshold=128)
@assert c >= 1 && b == 16 && t == 128
println("SFIDefectDetect OK")
'

echo "== defect-detect sidecar =="
julia --project=domains/industrial-inspection/plugins/defect-detect -e '
using Pkg
Pkg.instantiate()
include("domains/industrial-inspection/plugins/defect-detect/server_impl.jl")
println("defect-detect server_impl OK")
'

echo "== SFIVisionMeasure =="
julia --project=domains/industrial-inspection/plugins/defect-detect -e '
using Pkg
Pkg.instantiate()
include("domains/industrial-inspection/plugins/defect-detect/src/SFIVisionMeasure.jl")
using .SFIVisionMeasure
w, h = 128, 64
buf = Vector{UInt8}(undef, w * h)
for y in 0:(h - 1), x in 0:(w - 1)
    buf[y * w + x + 1] = x < 48 ? 30 : 220
end
buf[32 * w + 49] = 125
resp = process_measure_task(
    buf, w, h,
    Dict("measure" => Dict(
        "mmPerPixel" => 0.1,
        "edge" => Dict("x0" => 0, "y0" => 32, "x1" => 127, "polarity" => "rising"),
    ));
    task_id=1,
)
@assert resp["status"] == "ok"
@assert any(m -> m["name"] == "edge_position_px", resp["metrics"])
println("SFIVisionMeasure OK")
'

echo "== SFIVisionInspect =="
julia --project=domains/industrial-inspection/plugins/defect-detect -e '
using Pkg
Pkg.instantiate()
include("domains/industrial-inspection/plugins/defect-detect/src/SFIVisionInspect.jl")
using .SFIVisionInspect
w, h = 64, 48
buf = fill(UInt8(40), w * h)
        for y in 12:27, x in 20:35
            buf[y * w + x + 1] = 200
        end
        buf[12 * w + 21] = 180
resp = process_inspect_task(
    buf, w, h,
    Dict(
        "inspect" => Dict(
            "minScore" => 0.85,
            "search" => Dict("x0" => 0, "y0" => 0, "x1" => 63, "y1" => 47),
            "template" => Dict("x" => 20, "y" => 12, "width" => 16, "height" => 16),
        ),
        "calibration" => Dict("mmPerPixel" => 0.05),
    );
    task_id=1,
)
@assert resp["status"] == "ok"
@assert any(m -> m["name"] == "ncc_score", resp["metrics"])
println("SFIVisionInspect OK")
'

echo "== spc-metrics =="
julia --project=domains/industrial-inspection/plugins/spc-metrics -e '
using Pkg
Pkg.instantiate()
include("domains/industrial-inspection/plugins/spc-metrics/src/SFISpcMetrics.jl")
using SFISpcMetrics
@assert ng_rate([true, false, true]) ≈ 2/3
println("SFISpcMetrics OK")
'

echo "Julia checks OK"
