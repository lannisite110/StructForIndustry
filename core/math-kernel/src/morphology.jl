"""3×3 binary erosion (8-connected box structuring element)."""
function morph_erode_3x3(mask::AbstractVector{Bool}, width::Int, height::Int)
    n = width * height
    @assert length(mask) == n
    out = falses(n)
    for y in 0:(height - 1)
        for x in 0:(width - 1)
            i = y * width + x + 1
            if !mask[i]
                continue
            end
            ok = true
            for dy in -1:1
                for dx in -1:1
                    nx = x + dx
                    ny = y + dy
                    if nx < 0 || ny < 0 || nx >= width || ny >= height
                        ok = false
                        break
                    end
                    if !mask[ny * width + nx + 1]
                        ok = false
                        break
                    end
                end
                !ok && break
            end
            out[i] = ok
        end
    end
    return out
end

"""3×3 binary dilation (8-connected box)."""
function morph_dilate_3x3(mask::AbstractVector{Bool}, width::Int, height::Int)
    n = width * height
    @assert length(mask) == n
    out = falses(n)
    for y in 0:(height - 1)
        for x in 0:(width - 1)
            i = y * width + x + 1
            if mask[i]
                for dy in -1:1
                    for dx in -1:1
                        nx = x + dx
                        ny = y + dy
                        if nx >= 0 && ny >= 0 && nx < width && ny < height
                            out[ny * width + nx + 1] = true
                        end
                    end
                end
            end
        end
    end
    return out
end

function morph_open_3x3(mask::AbstractVector{Bool}, width::Int, height::Int)
    return morph_dilate_3x3(morph_erode_3x3(mask, width, height), width, height)
end

function morph_close_3x3(mask::AbstractVector{Bool}, width::Int, height::Int)
    return morph_erode_3x3(morph_dilate_3x3(mask, width, height), width, height)
end

function apply_morph(mask::AbstractVector{Bool}, width::Int, height::Int, mode::AbstractString)
    if mode == "open_3x3"
        return morph_open_3x3(mask, width, height)
    elseif mode == "close_3x3"
        return morph_close_3x3(mask, width, height)
    else
        return copy(mask)
    end
end
