"""Gray-level statistics: (min, max, mean, std)."""
function gray_stats(data::AbstractVector{UInt8})
    isempty(data) && return (0, 0, 0.0, 0.0)
    mn = minimum(data)
    mx = maximum(data)
    μ = mean(data)
    σ = std(data)
    return (Int(mn), Int(mx), Float64(μ), Float64(σ))
end

"""Histogram with `bins` buckets over [0, 255]."""
function gray_histogram(data::AbstractVector{UInt8}, bins::Int=256)
    h = zeros(Int, bins)
    scale = bins / 256.0
    for v in data
        idx = min(bins, max(1, floor(Int, v * scale) + 1))
        h[idx] += 1
    end
    return h
end
