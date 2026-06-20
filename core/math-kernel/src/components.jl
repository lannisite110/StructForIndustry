"""Per-pixel component labels (0 = background)."""
function connected_components_labels(mask::AbstractVector{Bool}, width::Int, height::Int)
    n = width * height
    @assert length(mask) == n
    labels = zeros(Int, n)
    current = 0
    for idx in 1:n
        mask[idx] || continue
        labels[idx] != 0 && continue
        current += 1
        stack = [idx]
        while !isempty(stack)
            i = pop!(stack)
            labels[i] != 0 && continue
            labels[i] = current
            mask[i] || continue
            x = (i - 1) % width
            y = (i - 1) ÷ width
            for (dx, dy) in ((0, -1), (0, 1), (-1, 0), (1, 0))
                nx = x + dx
                ny = y + dy
                (nx < 0 || ny < 0 || nx >= width || ny >= height) && continue
                j = ny * width + nx + 1
                if mask[j] && labels[j] == 0
                    push!(stack, j)
                end
            end
        end
    end
    return labels
end

"""
Blob stats per label: `(label, area, x, y, width, height, cx, cy)`.
"""
function blob_stats_from_labels(labels::AbstractVector{Int}, width::Int, height::Int)
    max_label = maximum(labels)
    max_label == 0 && return NamedTuple[]
    areas = zeros(Int, max_label)
    min_x = fill(width, max_label)
    min_y = fill(height, max_label)
    max_x = fill(-1, max_label)
    max_y = fill(-1, max_label)
    sum_x = zeros(Float64, max_label)
    sum_y = zeros(Float64, max_label)
    for idx in 1:length(labels)
        lbl = labels[idx]
        lbl == 0 && continue
        areas[lbl] += 1
        x = (idx - 1) % width
        y = (idx - 1) ÷ width
        min_x[lbl] = min(min_x[lbl], x)
        min_y[lbl] = min(min_y[lbl], y)
        max_x[lbl] = max(max_x[lbl], x)
        max_y[lbl] = max(max_y[lbl], y)
        sum_x[lbl] += x
        sum_y[lbl] += y
    end
    out = NamedTuple[]
    for lbl in 1:max_label
        areas[lbl] == 0 && continue
        area = areas[lbl]
        bx = min_x[lbl]
        by = min_y[lbl]
        bw = max_x[lbl] - bx + 1
        bh = max_y[lbl] - by + 1
        push!(
            out,
            (
                label=lbl,
                area=area,
                x=bx,
                y=by,
                width=bw,
                height=bh,
                cx=sum_x[lbl] / area,
                cy=sum_y[lbl] / area,
            ),
        )
    end
    return out
end

"""Filter blobs by area and aspect ratio (width/height, always ≥ 1)."""
function filter_blobs(
    blobs::AbstractVector{<:NamedTuple};
    min_area::Int=1,
    max_area::Int=typemax(Int),
    min_aspect::Float64=0.0,
    max_aspect::Float64=Inf,
)
    out = NamedTuple[]
    for b in blobs
        b.area < min_area && continue
        b.area > max_area && continue
        aspect = b.width >= b.height ? b.width / b.height : b.height / b.width
        aspect < min_aspect && continue
        aspect > max_aspect && continue
        push!(out, b)
    end
    return out
end

function largest_blob(blobs::AbstractVector{<:NamedTuple})
    isempty(blobs) && return nothing
    return blobs[argmax([b.area for b in blobs])]
end
