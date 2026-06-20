module SFIVisionInspect

using SFIMathKernel

export process_inspect_task, parse_inspect_params, mm_per_pixel_from_params

function parse_inspect_params(params::AbstractDict)
    inspect = get(params, "inspect", Dict())
    search = get(inspect, "search", Dict())
    tpl = get(inspect, "template", Dict())
    cal = get(params, "calibration", Dict())
    measure = get(params, "measure", Dict())
    mm = Float64(get(cal, "mmPerPixel", get(cal, "mm_per_pixel", 0.0)))
    if mm <= 0.0
        mm = Float64(get(measure, "mmPerPixel", get(measure, "mm_per_pixel", 0.0)))
    end
    return (
        search_x0=Int(get(search, "x0", 0)),
        search_y0=Int(get(search, "y0", 0)),
        search_x1=Int(get(search, "x1", 0)),
        search_y1=Int(get(search, "y1", 0)),
        tpl_x=Int(get(tpl, "x", 0)),
        tpl_y=Int(get(tpl, "y", 0)),
        tpl_w=Int(get(tpl, "width", 0)),
        tpl_h=Int(get(tpl, "height", 0)),
        min_score=Float64(get(inspect, "minScore", get(inspect, "min_score", 0.8))),
        expected_x=Float64(get(inspect, "expectedX", get(inspect, "expected_x", 0.0))),
        expected_y=Float64(get(inspect, "expectedY", get(inspect, "expected_y", 0.0))),
        position_tolerance=Float64(
            get(inspect, "positionTolerance", get(inspect, "position_tolerance", 0.0)),
        ),
        mm_per_pixel=mm,
    )
end

function mm_per_pixel_from_params(params::AbstractDict)
    return parse_inspect_params(params).mm_per_pixel
end

function process_inspect_task(
    pixels::AbstractVector{UInt8},
    width::Int,
    height::Int,
    params::AbstractDict;
    task_id::Integer,
    task_type::AbstractString="vision.inspect.template",
    message::AbstractString="inspect julia",
)
    cfg = parse_inspect_params(params)
    sx1 = cfg.search_x1 > 0 ? cfg.search_x1 : width - 1
    sy1 = cfg.search_y1 > 0 ? cfg.search_y1 : height - 1
    tpl_w = cfg.tpl_w > 0 ? cfg.tpl_w : 16
    tpl_h = cfg.tpl_h > 0 ? cfg.tpl_h : 16

    template, tw, th = extract_template(pixels, width, height, cfg.tpl_x, cfg.tpl_y, tpl_w, tpl_h)
    isempty(template) && return error_response(task_id, message, "invalid template roi")

    if task_type == "vision.inspect.presence"
        # Presence: template patch must correlate with itself at teach position.
        score = ncc_score_at(pixels, width, height, template, tw, th, cfg.tpl_x, cfg.tpl_y)
        return build_inspect_response(
            task_id, message, cfg.tpl_x, cfg.tpl_y, tw, th, score, cfg;
            label="presence",
        )
    end

    result = ncc_match(
        pixels,
        width,
        height,
        template,
        tw,
        th,
        cfg.search_x0,
        cfg.search_y0,
        sx1,
        sy1,
    )
    result === nothing && return error_response(task_id, message, "search area too small")
    match_x, match_y, score = result
    return build_inspect_response(
        task_id, message, match_x, match_y, tw, th, score, cfg;
        label="template",
    )
end

function build_inspect_response(
    task_id,
    message,
    match_x,
    match_y,
    tw,
    th,
    score,
    cfg;
    label::AbstractString="template",
)
    metrics = Dict{String, Any}[]
    push_metric!(metrics, "ncc_score", score, "ratio")
    push_metric!(metrics, "template_offset_x_px", Float64(match_x), "px")
    push_metric!(metrics, "template_offset_y_px", Float64(match_y), "px")
    if cfg.mm_per_pixel > 0.0
        push_metric!(metrics, "template_offset_x_mm", match_x * cfg.mm_per_pixel, "mm")
        push_metric!(metrics, "template_offset_y_mm", match_y * cfg.mm_per_pixel, "mm")
    end
    if cfg.expected_x > 0.0 || cfg.expected_y > 0.0
        dx = Float64(match_x) - cfg.expected_x
        dy = Float64(match_y) - cfg.expected_y
        push_metric!(metrics, "position_deviation_x_px", dx, "px")
        push_metric!(metrics, "position_deviation_y_px", dy, "px")
        if cfg.mm_per_pixel > 0.0
            push_metric!(metrics, "position_deviation_x_mm", dx * cfg.mm_per_pixel, "mm")
            push_metric!(metrics, "position_deviation_y_mm", dy * cfg.mm_per_pixel, "mm")
        end
    end

    status = score >= cfg.min_score ? "ok" : "error"
    msg = if status == "ok"
        message
    else
        "$message: ncc below min_score ($(score) < $(cfg.min_score))"
    end

    if status == "ok" && cfg.position_tolerance > 0.0
        for name in ["position_deviation_x_px", "position_deviation_y_px", "position_deviation_x_mm", "position_deviation_y_mm"]
            for m in metrics
                if get(m, "name", "") == name && abs(Float64(get(m, "value", 0.0))) > cfg.position_tolerance
                    status = "error"
                    msg = "$message: position tolerance exceeded (±$(cfg.position_tolerance))"
                    break
                end
            end
            status == "error" && break
        end
    end

    detection = Dict(
        "class_id" => 12,
        "label" => label,
        "score" => min(0.99, max(0.0, score)),
        "bbox" => Dict("x" => Float64(match_x), "y" => Float64(match_y), "width" => Float64(tw), "height" => Float64(th)),
    )

    return Dict(
        "task_id" => task_id,
        "status" => status,
        "message" => msg,
        "detections" => [detection],
        "metrics" => metrics,
    )
end

function push_metric!(metrics, name, value, unit)
    push!(metrics, Dict("name" => name, "value" => value, "unit" => unit))
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
