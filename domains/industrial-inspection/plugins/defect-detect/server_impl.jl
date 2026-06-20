using JSON3
using Mmap
using SFIDefectDetect
using Sockets

const WIRE_API_VERSION = 1

function read_framed_request(sock)
    len_bytes = read(sock, 4)
    length(len_bytes) < 4 && return nothing
    body_len = reinterpret(UInt32, len_bytes)[1]
    body = read(sock, body_len)
    return JSON3.read(String(body))
end

function write_framed_response(sock, resp)
    body = JSON3.write(resp)
    write(sock, reinterpret(UInt8, UInt32[UInt32(length(body))]))
    write(sock, body)
end

function shm_path(name::AbstractString)
    if startswith(name, "/dev/shm/")
        return name
    elseif startswith(name, "/") && occursin("/", name[2:end])
        return name
    elseif startswith(name, "/")
        return joinpath("/dev/shm", lstrip(name, '/'))
    else
        return joinpath("/dev/shm", name)
    end
end

function mmap_gray8(shm_name::AbstractString, byte_length::Integer, offset::Integer=0)
    path = shm_path(shm_name)
    io = open(path, "r")
    try
        bytes = Mmap.mmap(io, Mmap.Anonymous, byte_length, offset)
        return bytes
    finally
        close(io)
    end
end

function process_task(req)
    threshold = get(get(req, :params, Dict()), "threshold", 128)
    frame = req.frame
    pixels = mmap_gray8(frame.shm_name, frame.byte_length, get(frame, :offset, 0))
    width = Int(frame.width)
    height = Int(frame.height)

    components, bright, gmean, bbox = detect_surface_defects(
        pixels, width, height; threshold=threshold,
    )

    detections = []
    if components > 0
        bb = bbox === nothing ? (
            x=width * 0.25,
            y=height * 0.25,
            width=width * 0.5,
            height=height * 0.5,
        ) : bbox
        push!(detections, Dict(
            "class_id" => 1,
            "label" => "surface_defect",
            "score" => min(0.99, 0.5 + 0.1 * components),
            "bbox" => Dict(
                "x" => bb.x,
                "y" => bb.y,
                "width" => bb.width,
                "height" => bb.height,
            ),
        ))
    end

    return Dict(
        "task_id" => req.task_id,
        "status" => "ok",
        "message" => "defect-detect julia",
        "detections" => detections,
        "metrics" => [
            Dict("name" => "gray_mean", "value" => gmean, "unit" => "dn"),
            Dict("name" => "bright_pixels", "value" => bright, "unit" => "count"),
            Dict("name" => "defect_components", "value" => components, "unit" => "count"),
        ],
    )
end

function handle_client(sock)
    try
        while isopen(sock)
            req = read_framed_request(sock)
            req === nothing && break
            api = get(req, :api_version, 0)
            api != WIRE_API_VERSION && error("unsupported api_version $api")
            resp = process_task(req)
            write_framed_response(sock, resp)
        end
    catch err
        @warn "client error" exception=(err, catch_backtrace())
    finally
        close(sock)
    end
end
