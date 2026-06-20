"""Otsu optimal threshold on Gray8 data."""
function otsu_threshold(data::AbstractVector{UInt8})
    h = gray_histogram(data, 256)
    total = sum(h)
    total == 0 && return 128
    sum_all = sum((i - 1) * h[i] for i in 1:256)
    sum_b = 0.0
    w_b = 0
    best_var = -1.0
    best_t = 128
    for t in 1:255
        w_b += h[t]
        w_b == 0 && continue
        w_f = total - w_b
        w_f == 0 && break
        sum_b += (t - 1) * h[t]
        m_b = sum_b / w_b
        m_f = (sum_all - sum_b) / w_f
        var_between = w_b * w_f * (m_b - m_f)^2
        if var_between > best_var
            best_var = var_between
            best_t = t - 1
        end
    end
    return best_t
end

"""
Local mean adaptive threshold: foreground where `pixel >= local_mean - c`.
`block` must be odd (default 15). Uneven illumination without fixed threshold tuning.
"""
function adaptive_threshold(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int;
    block::Int=15,
    c::Int=5,
)
    n = width * height
    @assert length(data) == n
    half = block ÷ 2
    integral = zeros(Float64, (width + 1, height + 1))
    for y in 0:(height - 1)
        row_sum = 0.0
        for x in 0:(width - 1)
            row_sum += data[y * width + x + 1]
            integral[x + 2, y + 2] = integral[x + 2, y + 1] + row_sum
        end
    end
    mask = falses(n)
    for y in 0:(height - 1)
        y0 = max(0, y - half)
        y1 = min(height - 1, y + half)
        for x in 0:(width - 1)
            x0 = max(0, x - half)
            x1 = min(width - 1, x + half)
            area = (x1 - x0 + 1) * (y1 - y0 + 1)
            sum_rect = integral[x1 + 2, y1 + 2] - integral[x0 + 1, y1 + 2] -
                       integral[x1 + 2, y0 + 1] + integral[x0 + 1, y0 + 1]
            local_mean = sum_rect / area
            idx = y * width + x + 1
            mask[idx] = data[idx] >= local_mean - c
        end
    end
    return mask
end

function threshold_mask(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int,
    mode::AbstractString,
    fixed::Integer,
)
    if mode == "otsu"
        t = otsu_threshold(data)
        return gray_threshold(data, t), t
    elseif mode == "adaptive"
        return adaptive_threshold(data, width, height), fixed
    else
        return gray_threshold(data, fixed), fixed
    end
end
