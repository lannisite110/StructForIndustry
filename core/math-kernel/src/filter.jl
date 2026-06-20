"""Separable 3×3 Gaussian blur (kernel [1,2,1]/4), returns new UInt8 buffer."""
function gaussian_blur_3x3(data::AbstractVector{UInt8}, width::Int, height::Int)
    n = width * height
    @assert length(data) == n
    tmp = Vector{Float32}(undef, n)
    out = Vector{UInt8}(undef, n)
    # horizontal
    for y in 0:(height - 1)
        row = y * width
        for x in 0:(width - 1)
            i = row + x + 1
            l = x > 0 ? data[row + x] : data[i]
            c = data[i]
            r = x + 1 < width ? data[row + x + 2] : c
            tmp[i] = (l + 2.0f0 * c + r) * 0.25f0
        end
    end
    # vertical
    for y in 0:(height - 1)
        for x in 0:(width - 1)
            i = y * width + x + 1
            u = y > 0 ? tmp[(y - 1) * width + x + 1] : tmp[i]
            c = tmp[i]
            d = y + 1 < height ? tmp[(y + 1) * width + x + 1] : c
            v = (u + 2.0f0 * c + d) * 0.25f0
            out[i] = clamp(round(UInt8, v), 0x00, 0xff)
        end
    end
    return out
end

"""3×3 median filter."""
function median_filter_3x3(data::AbstractVector{UInt8}, width::Int, height::Int)
    n = width * height
    @assert length(data) == n
    out = Vector{UInt8}(undef, n)
    buf = Vector{UInt8}(undef, 9)
    for y in 0:(height - 1)
        for x in 0:(width - 1)
            k = 0
            for dy in -1:1
                for dx in -1:1
                    nx = clamp(x + dx, 0, width - 1)
                    ny = clamp(y + dy, 0, height - 1)
                    k += 1
                    buf[k] = data[ny * width + nx + 1]
                end
            end
            sort!(buf, 1, 9)
            out[y * width + x + 1] = buf[5]
        end
    end
    return out
end

function apply_preproc(
    data::AbstractVector{UInt8},
    width::Int,
    height::Int,
    mode::AbstractString,
)
    if mode == "gaussian_3x3"
        return gaussian_blur_3x3(data, width, height)
    elseif mode == "median_3x3"
        return median_filter_3x3(data, width, height)
    else
        return copy(data)
    end
end
