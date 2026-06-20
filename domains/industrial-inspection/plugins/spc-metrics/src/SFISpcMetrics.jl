module SFISpcMetrics

using Statistics

export rolling_mean, rolling_std, rolling_minmax, gray_mean, ng_rate, cp_cpk, histogram_peak

"""Rolling mean over the last `window` samples."""
function rolling_mean(samples::Vector{Float64}, window::Int)
    n = length(samples)
    n == 0 && return Float64[]
    window = max(1, window)
    out = Float64[]
    for i in 1:n
        lo = max(1, i - window + 1)
        push!(out, mean(samples[lo:i]))
    end
    return out
end

"""Rolling std over the last `window` samples."""
function rolling_std(samples::Vector{Float64}, window::Int)
    n = length(samples)
    n == 0 && return Float64[]
    window = max(1, window)
    out = Float64[]
    for i in 1:n
        lo = max(1, i - window + 1)
        slice = samples[lo:i]
        push!(out, length(slice) < 2 ? 0.0 : std(slice))
    end
    return out
end

"""Rolling (min, max) pairs over the last `window` samples."""
function rolling_minmax(samples::Vector{Float64}, window::Int)
    n = length(samples)
    n == 0 && return Tuple{Float64, Float64}[]
    window = max(1, window)
    out = Tuple{Float64, Float64}[]
    for i in 1:n
        lo = max(1, i - window + 1)
        slice = samples[lo:i]
        push!(out, (minimum(slice), maximum(slice)))
    end
    return out
end

function gray_mean(pixels::AbstractVector{UInt8})
    return mean(pixels)
end

function ng_rate(results::AbstractVector{Bool})
    isempty(results) && return 0.0
    return count(identity, results) / length(results)
end

"""Process capability indices from mean, sigma, USL, LSL."""
function cp_cpk(mean::Float64, sigma::Float64, usl::Float64, lsl::Float64)
    sigma < 1e-9 || usl <= lsl && return (NaN, NaN)
    cp = (usl - lsl) / (6.0 * sigma)
    cpu = (usl - mean) / (3.0 * sigma)
    cpl = (mean - lsl) / (3.0 * sigma)
    return (cp, min(cpu, cpl))
end

"""Peak histogram bin center (DN) and fraction for values in [0,255]."""
function histogram_peak(samples::AbstractVector{Float64}, bins::Int=16)
    isempty(samples) && return (0.0, 0.0)
    bins = max(4, bins)
    counts = zeros(Int, bins)
    for v in samples
        idx = min(bins, max(1, floor(Int, v / 256.0 * bins) + 1))
        counts[idx] += 1
    end
    peak_idx = argmax(counts)
    peak_dn = (peak_idx - 0.5) * (256.0 / bins)
    ratio = counts[peak_idx] / length(samples)
    return (peak_dn, ratio)
end

end
