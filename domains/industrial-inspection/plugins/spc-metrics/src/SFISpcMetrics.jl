module SFISpcMetrics

using Statistics

export rolling_mean, gray_mean, ng_rate

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

function gray_mean(pixels::AbstractVector{UInt8})
    return mean(pixels)
end

function ng_rate(results::AbstractVector{Bool})
    isempty(results) && return 0.0
    return count(identity, results) / length(results)
end

end
