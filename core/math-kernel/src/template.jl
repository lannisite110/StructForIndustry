"""Zero-mean normalized cross-correlation between template and image patch at (x, y)."""
function ncc_score_at(
    image::AbstractVector{UInt8},
    width::Int,
    height::Int,
    template::AbstractVector{UInt8},
    tw::Int,
    th::Int,
    x::Int,
    y::Int,
)
    tw < 1 || th < 1 && return -1.0
    x < 0 || y < 0 || x + tw > width || y + th > height && return -1.0
    n = tw * th
    μ_t = mean(template)
    σ_t = std(template)
    σ_t < 1e-6 && return -1.0
    sum_cross = 0.0
    sum_i = 0.0
    sum_i2 = 0.0
    for row in 0:(th - 1)
        img_row = (y + row) * width + x
        tpl_row = row * tw
        for col in 0:(tw - 1)
            iv = Float64(image[img_row + col + 1])
            tv = Float64(template[tpl_row + col + 1])
            sum_cross += (tv - μ_t) * iv
            sum_i += iv
            sum_i2 += iv * iv
        end
    end
    μ_i = sum_i / n
    var_i = sum_i2 / n - μ_i * μ_i
    σ_i = sqrt(max(var_i, 0.0))
    σ_i < 1e-6 && return -1.0
    return sum_cross / (n * σ_t * σ_i)
end

"""
Search `image` for best NCC match of `template` inside `[x0,x1]×[y0,y1]`.
Returns `(best_x, best_y, best_score)` or `nothing` if search area too small.
"""
function ncc_match(
    image::AbstractVector{UInt8},
    width::Int,
    height::Int,
    template::AbstractVector{UInt8},
    tw::Int,
    th::Int,
    x0::Int,
    y0::Int,
    x1::Int,
    y1::Int;
    step::Int=1,
)
    tw < 1 || th < 1 && return nothing
    step = max(1, step)
    x0 = clamp(x0, 0, width - tw)
    y0 = clamp(y0, 0, height - th)
    x1 = clamp(x1, 0, width - tw)
    y1 = clamp(y1, 0, height - th)
    x0 > x1 && (x0, x1) = (x1, x0)
    y0 > y1 && (y0, y1) = (y1, y0)
    best_x, best_y = x0, y0
    best_score = -1.0
    for y in y0:step:y1
        for x in x0:step:x1
            score = ncc_score_at(image, width, height, template, tw, th, x, y)
            if score > best_score
                best_score = score
                best_x, best_y = x, y
            end
        end
    end
    best_score < -0.5 && return nothing
    return (best_x, best_y, best_score)
end

"""Extract row-major template from image rectangle."""
function extract_template(
    image::AbstractVector{UInt8},
    width::Int,
    height::Int,
    tx::Int,
    ty::Int,
    tw::Int,
    th::Int,
)
    tx = clamp(tx, 0, width - 1)
    ty = clamp(ty, 0, height - 1)
    tw = min(tw, width - tx)
    th = min(th, height - ty)
    tw < 1 || th < 1 && return UInt8[], 0, 0
    out = Vector{UInt8}(undef, tw * th)
    k = 1
    for row in 0:(th - 1)
        start = (ty + row) * width + tx
        for col in 0:(tw - 1)
            out[k] = image[start + col + 1]
            k += 1
        end
    end
    return out, tw, th
end
