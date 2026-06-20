"""Sobel gradient magnitude (3×3), row-major Gray8 → Float32 magnitudes."""
function sobel_magnitude(data::AbstractVector{UInt8}, width::Int, height::Int)
    n = width * height
    @assert length(data) == n
    out = Vector{Float32}(undef, n)
    for y in 0:(height - 1)
        for x in 0:(width - 1)
            i = y * width + x + 1
            at = (xx, yy) -> data[clamp(yy, 0, height - 1) * width + clamp(xx, 0, width - 1) + 1]
            gx = -at(x - 1, y - 1) - 2 * at(x - 1, y) - at(x - 1, y + 1) +
                 at(x + 1, y - 1) + 2 * at(x + 1, y) + at(x + 1, y + 1)
            gy = -at(x - 1, y - 1) - 2 * at(x, y - 1) - at(x + 1, y - 1) +
                 at(x - 1, y + 1) + 2 * at(x, y + 1) + at(x + 1, y + 1)
            out[i] = sqrt(Float32(gx * gx + gy * gy))
        end
    end
    return out
end

"""Parabolic sub-pixel offset from three samples around peak (returns offset in [-1,1])."""
function parabolic_subpixel(y0::Float64, y1::Float64, y2::Float64)
    denom = y0 - 2.0 * y1 + y2
    abs(denom) < 1e-6 && return 0.0
    return 0.5 * (y0 - y2) / denom
end

"""
Horizontal edge caliper on row `y` between `x0` and `x1`.
`polarity`: `rising` (dark→bright), `falling`, or `both` (strongest).
Returns `(subpixel_x, gradient_strength)` or `nothing` if no edge.
"""
function edge_caliper_horizontal(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int,
    y::Int,
    x0::Int,
    x1::Int,
    polarity::AbstractString="rising",
)
    y = clamp(y, 0, height - 1)
    x0 = clamp(x0, 0, width - 1)
    x1 = clamp(x1, 0, width - 1)
    x0 > x1 && (x0, x1) = (x1, x0)
    len = x1 - x0
    len < 2 && return nothing
    row = y * width
    best_i = 0
    best_g = -1.0
    for x in x0:(x1 - 1)
        g = Float64(data[row + x + 2]) - Float64(data[row + x + 1])
        mag = if polarity == "falling"
            -g
        elseif polarity == "both"
            abs(g)
        else
            g # rising
        end
        if mag > best_g
            best_g = mag
            best_i = x
        end
    end
    best_g <= 0.0 && return nothing
    # sub-pixel on gradient profile
    i = best_i
    g0 = Float64(data[row + i + 1]) - Float64(data[row + i])
    g1 = Float64(data[row + i + 2]) - Float64(data[row + i + 1])
    g2 = if i + 2 < x1
        Float64(data[row + i + 3]) - Float64(data[row + i + 2])
    else
        g1
    end
    if polarity == "falling"
        g0, g1, g2 = -g0, -g1, -g2
    elseif polarity == "both"
        g0, g1, g2 = abs(g0), abs(g1), abs(g2)
    end
    sub = parabolic_subpixel(g0, g1, g2)
    return (Float64(i) + 0.5 + sub, best_g)
end

"""Vertical edge caliper on column `x` between `y0` and `y1`."""
function edge_caliper_vertical(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int,
    x::Int,
    y0::Int,
    y1::Int,
    polarity::AbstractString="rising",
)
    x = clamp(x, 0, width - 1)
    y0 = clamp(y0, 0, height - 1)
    y1 = clamp(y1, 0, height - 1)
    y0 > y1 && (y0, y1) = (y1, y0)
    y1 - y0 < 2 && return nothing
    best_i = 0
    best_g = -1.0
    for y in y0:(y1 - 1)
        g = Float64(data[(y + 1) * width + x + 1]) - Float64(data[y * width + x + 1])
        mag = polarity == "falling" ? -g : (polarity == "both" ? abs(g) : g)
        if mag > best_g
            best_g = mag
            best_i = y
        end
    end
    best_g <= 0.0 && return nothing
    i = best_i
    g0 = Float64(data[(i + 1) * width + x + 1]) - Float64(data[i * width + x + 1])
    g1 = Float64(data[(i + 2) * width + x + 1]) - Float64(data[(i + 1) * width + x + 1])
    g2 = if i + 2 < y1
        Float64(data[(i + 3) * width + x + 1]) - Float64(data[(i + 2) * width + x + 1])
    else
        g1
    end
    if polarity == "falling"
        g0, g1, g2 = -g0, -g1, -g2
    elseif polarity == "both"
        g0, g1, g2 = abs(g0), abs(g1), abs(g2)
    end
    sub = parabolic_subpixel(g0, g1, g2)
    return (Float64(i) + 0.5 + sub, best_g)
end

"""Least-squares line fit `y = slope * x + intercept` for point pairs."""
function fit_line(points::AbstractVector{<:Tuple{Float64, Float64}})
    isempty(points) && return (0.0, 0.0)
    n = length(points)
    sx, sy, sxx, sxy = 0.0, 0.0, 0.0, 0.0
    for (x, y) in points
        sx += x
        sy += y
        sxx += x * x
        sxy += x * y
    end
    denom = n * sxx - sx * sx
    abs(denom) < 1e-9 && return (0.0, sy / n)
    slope = (n * sxy - sx * sy) / denom
    intercept = (sy - slope * sx) / n
    return (slope, intercept)
end

"""Algebraic circle fit (Kasa); returns `(cx, cy, radius)` or `nothing`."""
function fit_circle(points::AbstractVector{<:Tuple{Float64, Float64}})
    length(points) < 3 && return nothing
    n = length(points)
    sum_x, sum_y = 0.0, 0.0
    sum_x2, sum_y2, sum_xy = 0.0, 0.0, 0.0
    sum_x3, sum_y3 = 0.0, 0.0
    sum_x1y2, sum_x2y1 = 0.0, 0.0
    for (x, y) in points
        x2 = x * x
        y2 = y * y
        sum_x += x
        sum_y += y
        sum_x2 += x2
        sum_y2 += y2
        sum_xy += x * y
        sum_x3 += x2 * x
        sum_y3 += y2 * y
        sum_x1y2 += x * y2
        sum_x2y1 += x2 * y
    end
  A = [
        sum_x2 sum_xy sum_x
        sum_xy sum_y2 sum_y
        sum_x sum_y n
    ]
    b = [-sum_x3 - sum_x1y2, -sum_x2y1 - sum_y3, -(sum_x2 + sum_y2)]
    try
        sol = A \ b
        cx = -0.5 * sol[1]
        cy = -0.5 * sol[2]
        r = sqrt(max(cx * cx + cy * cy - sol[3], 0.0))
        return (cx, cy, r)
    catch
        return nothing
    end
end

function distance_point_to_line(px::Float64, py::Float64, slope::Float64, intercept::Float64)
    # line y = m*x + c → ax + by + c = 0  =>  m*x - y + c = 0
    a, b, c = slope, -1.0, intercept
    denom = sqrt(a * a + b * b)
    denom < 1e-9 && return abs(py - intercept)
    return abs(a * px + b * py + c) / denom
end

"""Horizontal line width: left rising edge + right falling edge on row `y`."""
function measure_line_width_horizontal(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int,
    y::Int,
    x0::Int,
    x1::Int,
)
    left = edge_caliper_horizontal(data, width, height, y, x0, x1, "rising")
    right = edge_caliper_horizontal(data, width, height, y, x0, x1, "falling")
    if left === nothing || right === nothing
        return nothing
    end
    w = right[1] - left[1]
    w <= 0.0 && return nothing
    return (w, left[1], right[1], (left[2] + right[2]) / 2.0)
end

"""Disk diameter via horizontal caliper through `(cx, cy)` (rising + falling)."""
function measure_circle_diameter_horizontal(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int,
    cy::Int,
    x0::Int,
    x1::Int,
)
    w = measure_line_width_horizontal(data, width, height, cy, x0, x1)
    w === nothing && return nothing
    return (w[1], w[1] / 2.0, w[2], w[3]) # diameter, radius, left_x, right_x
end
