module SFIMathKernel

using Statistics

include("stats.jl")
include("filter.jl")
include("threshold.jl")
include("morphology.jl")
include("components.jl")
include("measure.jl")

export gray_threshold,
    bright_pixel_count,
    connected_components_count,
    gray_stats,
    gray_histogram,
    otsu_threshold,
    adaptive_threshold,
    threshold_mask,
    gaussian_blur_3x3,
    median_filter_3x3,
    apply_preproc,
    morph_erode_3x3,
    morph_dilate_3x3,
    morph_open_3x3,
    morph_close_3x3,
    apply_morph,
    connected_components_labels,
    blob_stats_from_labels,
    filter_blobs,
    largest_blob,
    sobel_magnitude,
    parabolic_subpixel,
    edge_caliper_horizontal,
    edge_caliper_vertical,
    fit_line,
    fit_circle,
    distance_point_to_line,
    measure_line_width_horizontal,
    measure_circle_diameter_horizontal

"""
    gray_threshold(data, threshold) -> BitVector

Return a boolean mask where pixel value >= threshold (Gray8).
"""
function gray_threshold(data::AbstractVector{UInt8}, threshold::Integer)
    return data .>= threshold
end

"""
    bright_pixel_count(data, threshold) -> Int

Count pixels at or above threshold.
"""
function bright_pixel_count(data::AbstractVector{UInt8}, threshold::Integer)
    return count(>=(threshold), data)
end

"""
    connected_components_count(mask, width, height) -> Int

4-connected component count on a row-major boolean mask.
"""
function connected_components_count(mask::AbstractVector{Bool}, width::Int, height::Int)
    labels = connected_components_labels(mask, width, height)
    return maximum(labels)
end

end # module
