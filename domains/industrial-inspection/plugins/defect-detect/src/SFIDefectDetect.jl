module SFIDefectDetect

using SFIMathKernel
using Statistics

export detect_surface_defects, gray_mean, defect_bbox_from_mask

"""
    gray_mean(pixels) -> Float64
"""
gray_mean(pixels::AbstractVector{UInt8}) = mean(pixels)

"""
    defect_bbox_from_mask(mask, width, height) -> NamedTuple or nothing

Bounding box of foreground (true) pixels in row-major mask.
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

"""
    detect_surface_defects(pixels, width, height; threshold=128)

Industrial surface defect pipeline: threshold + components + bbox.
Returns (defect_count, bright_pixels, gray_mean, bbox_or_nothing).
"""
function detect_surface_defects(
    pixels::AbstractVector{UInt8},
    width::Int,
    height::Int;
    threshold::Integer=128,
)
    mask = gray_threshold(pixels, threshold)
    components = connected_components_count(mask, width, height)
    bright = bright_pixel_count(pixels, threshold)
    gmean = gray_mean(pixels)
    bbox = components > 0 ? defect_bbox_from_mask(mask, width, height) : nothing
    return components, bright, gmean, bbox
end

end
