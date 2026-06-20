module SFIDefectDetect

using SFIMathKernel
using Statistics

export detect_surface_defects,
    process_defect_task,
    gray_mean,
    defect_bbox_from_mask,
    parse_algorithm_params

"""
    gray_mean(pixels) -> Float64
"""
gray_mean(pixels::AbstractVector{UInt8}) = mean(pixels)

"""
    defect_bbox_from_mask(mask, width, height) -> NamedTuple or nothing
"""
function defect_bbox_from_mask(mask::AbstractVector{Bool}, width::Int, height::Int)
    n = length(mask)
    @assert n == width * height
    min_x, min_y = width, height
    max_x, max_y = -1, -1
    for idx in 1:n
        mask[idx] || continue
        x = (idx - 1) % width
        y = (idx - 1) ÷ width
        min_x = min(min_x, x)
        min_y = min(min_y, y)
        max_x = max(max_x, x)
        max_y = max(max_y, y)
    end
    max_x < 0 && return nothing
    return (x=min_x, y=min_y, width=max_x - min_x + 1, height=max_y - min_y + 1)
end

"""Parse `algorithm` block from task params (JSON camelCase or snake_case)."""
function parse_algorithm_params(params::AbstractDict)
    algo = get(params, "algorithm", Dict())
    blob = get(algo, "blob", Dict())
    preproc = string(get(algo, "preproc", "none"))
    threshold_mode = string(get(algo, "thresholdMode", get(algo, "threshold_mode", "fixed")))
    morph = string(get(algo, "morph", "none"))
    min_area = Int(get(blob, "minArea", get(blob, "min_area", 1)))
    max_area = Int(get(blob, "maxArea", get(blob, "max_area", typemax(Int))))
    min_aspect = Float64(get(blob, "minAspect", get(blob, "min_aspect", 0.0)))
    max_aspect = Float64(get(blob, "maxAspect", get(blob, "max_aspect", Inf)))
    return (
        preproc=preproc,
        threshold_mode=threshold_mode,
        morph=morph,
        min_area=min_area,
        max_area=max_area,
        min_aspect=min_aspect,
        max_aspect=max_aspect,
    )
end

"""
Industrial surface defect pipeline (Phase A):
preproc → threshold (fixed/otsu/adaptive) → morphology → blob filter.

Returns `(defect_count, bright_pixels, gray_mean, bbox_or_nothing, used_threshold)`.
"""
function detect_surface_defects(
    pixels::AbstractVector{UInt8},
    width::Int,
    height::Int;
    threshold::Integer=128,
    preproc::AbstractString="none",
    threshold_mode::AbstractString="fixed",
    morph::AbstractString="none",
    min_area::Int=1,
    max_area::Int=typemax(Int),
    min_aspect::Float64=0.0,
    max_aspect::Float64=Inf,
)
    work = apply_preproc(pixels, width, height, preproc)
    mask, used_t = threshold_mask(work, width, height, threshold_mode, threshold)
    mask = apply_morph(mask, width, height, morph)
    labels = connected_components_labels(mask, width, height)
    blobs = blob_stats_from_labels(labels, width, height)
    kept = filter_blobs(
        blobs;
        min_area=min_area,
        max_area=max_area,
        min_aspect=min_aspect,
        max_aspect=max_aspect,
    )
    bright = bright_pixel_count(work, used_t)
    gmean = gray_mean(work)
    best = largest_blob(kept)
    bbox = best === nothing ? nothing : (x=best.x, y=best.y, width=best.width, height=best.height)
    return length(kept), bright, gmean, bbox, used_t
end

"""
Build plugin wire response dict from pixels + task params (shared by defect-detect / vision-2d).
"""
function process_defect_task(
    pixels::AbstractVector{UInt8},
    width::Int,
    height::Int,
    params::AbstractDict;
    task_id::Integer,
    message::AbstractString="defect-detect julia",
    label::AbstractString="surface_defect",
)
    threshold = Int(get(params, "threshold", 128))
    algo = parse_algorithm_params(params)
    components, bright, gmean, bbox, used_t = detect_surface_defects(
        pixels,
        width,
        height;
        threshold=threshold,
        preproc=algo.preproc,
        threshold_mode=algo.threshold_mode,
        morph=algo.morph,
        min_area=algo.min_area,
        max_area=algo.max_area,
        min_aspect=algo.min_aspect,
        max_aspect=algo.max_aspect,
    )

    detections = []
    if components > 0
        bb = bbox === nothing ? (
            x=width * 0.25,
            y=height * 0.25,
            width=width * 0.5,
            height=height * 0.5,
        ) : bbox
        push!(detections, Dict(
            "class_id" => 1,
            "label" => label,
            "score" => min(0.99, 0.5 + 0.1 * components),
            "bbox" => Dict(
                "x" => bb.x,
                "y" => bb.y,
                "width" => bb.width,
                "height" => bb.height,
            ),
        ))
    end

    return Dict(
        "task_id" => task_id,
        "status" => "ok",
        "message" => message,
        "detections" => detections,
        "metrics" => [
            Dict("name" => "gray_mean", "value" => gmean, "unit" => "dn"),
            Dict("name" => "bright_pixels", "value" => bright, "unit" => "count"),
            Dict("name" => "defect_components", "value" => components, "unit" => "count"),
            Dict("name" => "threshold_used", "value" => used_t, "unit" => "dn"),
        ],
    )
end

end
