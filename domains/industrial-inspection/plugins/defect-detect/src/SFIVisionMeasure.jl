module SFIVisionMeasure

using SFIMathKernel

export process_measure_task, parse_measure_params

function parse_measure_params(params::AbstractDict)
    measure = get(params, "measure", Dict())
    edge = get(measure, "edge", Dict())
    dim = get(measure, "dimension", Dict())
    cal = get(params, "calibration", Dict())
    mm = Float64(get(measure, "mmPerPixel", get(measure, "mm_per_pixel", 0.0)))
    if mm <= 0.0
        mm = Float64(get(cal, "mmPerPixel", get(cal, "mm_per_pixel", 0.0)))
    end
    return (
        mm_per_pixel=mm,
        x0=Int(get(edge, "x0", 0)),
        y0=Int(get(edge, "y0", 0)),
        x1=Int(get(edge, "x1", 0)),
        y1=Int(get(edge, "y1", 0)),
        polarity=string(get(edge, "polarity", "rising")),
        dim_kind=string(get(dim, "kind", "edge_position")),
        nominal=Float64(get(dim, "nominal", 0.0)),
        tolerance=Float64(get(dim, "tolerance", 0.0)),
    )
end

function tolerance_violation(metrics, tolerance::Float64, names::Vector{String})
    tolerance <= 0.0 && return false
    for name in names
        for m in metrics
            if get(m, "name", "") == name
                abs(Float64(get(m, "value", 0.0))) > tolerance
                return true
            end
        end
    end
    return false
end

function finalize_measure_response(resp, cfg, message)
    names = String[
        "dimension_deviation_px",
        "dimension_deviation_mm",
        "edge_deviation_px",
        "edge_deviation_mm",
    ]
    if tolerance_violation(resp["metrics"], cfg.tolerance, names)
        resp["status"] = "error"
        resp["message"] = "$(message): tolerance exceeded (±$(cfg.tolerance))"
    end
    return resp
end

function process_measure_task(
    pixels::AbstractVector{UInt8},
    width::Int,
    height::Int,
    params::AbstractDict;
    task_id::Integer,
    task_type::AbstractString="vision.measure.edge",
    message::AbstractString="measure julia",
)
    cfg = parse_measure_params(params)
    x1 = cfg.x1 > 0 ? cfg.x1 : width - 1
    y1 = cfg.y1 > 0 ? cfg.y1 : cfg.y0

    metrics = Dict{String, Any}[]
    detections = []

    if task_type == "vision.measure.dimension"
        if cfg.dim_kind == "line_width"
            result = measure_line_width_horizontal(pixels, width, height, cfg.y0, cfg.x0, x1)
            result === nothing && return error_response(task_id, message, "no edges for line_width")
            width_px, left_x, right_x, strength = result
            push_metric!(metrics, "line_width_px", width_px, "px")
            if cfg.mm_per_pixel > 0.0
                push_metric!(metrics, "line_width_mm", width_px * cfg.mm_per_pixel, "mm")
            end
            push_metric!(metrics, "edge_strength", strength, "dn")
            if cfg.nominal > 0.0
                dev = width_px - cfg.nominal
                push_metric!(metrics, "dimension_deviation_px", dev, "px")
                cfg.mm_per_pixel > 0.0 &&
                    push_metric!(metrics, "dimension_deviation_mm", dev * cfg.mm_per_pixel, "mm")
            end
            push!(detections, line_detection(left_x, cfg.y0, right_x - left_x, 1.0, "line_width"))
        elseif cfg.dim_kind == "circle_diameter"
            result = measure_circle_diameter_horizontal(pixels, width, height, cfg.y0, cfg.x0, x1)
            result === nothing && return error_response(task_id, message, "no circle diameter")
            diam, radius, left_x, right_x = result
            push_metric!(metrics, "circle_diameter_px", diam, "px")
            push_metric!(metrics, "circle_radius_px", radius, "px")
            if cfg.mm_per_pixel > 0.0
                push_metric!(metrics, "circle_diameter_mm", diam * cfg.mm_per_pixel, "mm")
            end
            if cfg.nominal > 0.0
                dev = diam - cfg.nominal
                push_metric!(metrics, "dimension_deviation_px", dev, "px")
            end
            cx = (left_x + right_x) / 2.0
            push!(detections, line_detection(cx - radius, cfg.y0 - 1.0, diam, 2.0, "circle"))
        else
            # edge_position as dimension
            edge_result = measure_edge(pixels, width, height, cfg, x1, y1)
            edge_result === nothing && return error_response(task_id, message, "no edge")
            pos, strength = edge_result
            push_edge_metrics!(metrics, pos, strength, cfg)
            push!(detections, point_detection(pos, cfg.y0, strength, "edge"))
        end
    else
        edge_result = measure_edge(pixels, width, height, cfg, x1, y1)
        edge_result === nothing && return error_response(task_id, message, "no edge")
        pos, strength = edge_result
        push_edge_metrics!(metrics, pos, strength, cfg)
        push!(detections, point_detection(pos, cfg.y0, strength, "edge"))
    end

    return finalize_measure_response(
        Dict(
            "task_id" => task_id,
            "status" => "ok",
            "message" => message,
            "detections" => detections,
            "metrics" => metrics,
        ),
        cfg,
        message,
    )
end

function measure_edge(pixels, width, height, cfg, x1, y1)
    if cfg.y0 == y1
        return edge_caliper_horizontal(
            pixels, width, height, cfg.y0, cfg.x0, x1, cfg.polarity,
        )
    elseif cfg.x0 == x1
        return edge_caliper_vertical(
            pixels, width, height, cfg.x0, cfg.y0, y1, cfg.polarity,
        )
    else
        return edge_caliper_horizontal(
            pixels, width, height, cfg.y0, cfg.x0, x1, cfg.polarity,
        )
    end
end

function push_metric!(metrics, name, value, unit)
    push!(metrics, Dict("name" => name, "value" => value, "unit" => unit))
end

function push_edge_metrics!(metrics, pos, strength, cfg)
    push_metric!(metrics, "edge_position_px", pos, "px")
    push_metric!(metrics, "edge_strength", strength, "dn")
    if cfg.mm_per_pixel > 0.0
        push_metric!(metrics, "edge_position_mm", pos * cfg.mm_per_pixel, "mm")
    end
    if cfg.nominal > 0.0
        dev = pos - cfg.nominal
        push_metric!(metrics, "edge_deviation_px", dev, "px")
        cfg.mm_per_pixel > 0.0 && push_metric!(metrics, "edge_deviation_mm", dev * cfg.mm_per_pixel, "mm")
    end
end

function point_detection(x, y, score, label)
    return Dict(
        "class_id" => 10,
        "label" => label,
        "score" => min(0.99, score / 255.0),
        "bbox" => Dict("x" => x - 1.0, "y" => y - 1.0, "width" => 2.0, "height" => 2.0),
    )
end

function line_detection(x, y, w, h, label)
    return Dict(
        "class_id" => 11,
        "label" => label,
        "score" => 0.9,
        "bbox" => Dict("x" => x, "y" => y, "width" => w, "height" => h),
    )
end

function error_response(task_id, message, err)
    return Dict(
        "task_id" => task_id,
        "status" => "error",
        "message" => "$message: $err",
        "detections" => [],
        "metrics" => [],
    )
end

end
