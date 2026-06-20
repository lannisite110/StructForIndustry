module SFIMathKernel

using Statistics

export gray_threshold, bright_pixel_count, connected_components_count

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

Simple 4-connected component count on a row-major boolean mask (Phase 2 stub).
"""
function connected_components_count(mask::AbstractVector{Bool}, width::Int, height::Int)
    n = length(mask)
    @assert n == width * height
    visited = falses(n)
    components = 0
    for idx in 1:n
        mask[idx] || continue
        visited[idx] && continue
        components += 1
        stack = [idx]
        while !isempty(stack)
            i = pop!(stack)
            visited[i] && continue
            visited[i] = true
            mask[i] || continue
            x = (i - 1) % width
            y = (i - 1) ÷ width
            for (dx, dy) in ((0, -1), (0, 1), (-1, 0), (1, 0))
                nx, ny = x + dx, y + dy
                (nx < 0 || ny < 0 || nx >= width || ny >= height) && continue
                j = ny * width + nx + 1
                !visited[j] && mask[j] && push!(stack, j)
            end
        end
    end
    return components
end

end # module
