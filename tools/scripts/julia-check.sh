#!/usr/bin/env bash
# Julia syntax / load check for math-kernel and vision-2d
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "== math-kernel =="
julia --project=core/math-kernel -e '
using Pkg
Pkg.instantiate()
include("core/math-kernel/src/SFIMathKernel.jl")
using SFIMathKernel
@assert bright_pixel_count(UInt8[1, 200, 3], 128) == 1
println("SFIMathKernel OK")
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
c, b, g, bb = detect_surface_defects(fill(UInt8(200), 16), 4, 4; threshold=128)
@assert c >= 1 && b == 16
println("SFIDefectDetect OK")
'

echo "== defect-detect sidecar =="
julia --project=domains/industrial-inspection/plugins/defect-detect -e '
using Pkg
Pkg.instantiate()
include("domains/industrial-inspection/plugins/defect-detect/server_impl.jl")
println("defect-detect server_impl OK")
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
