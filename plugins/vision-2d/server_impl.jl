using JSON3
using Mmap
using SFIMathKernel
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

function mmap_gray8(shm_name::AbstractString, byte_length::Integer, offset::Integer=0)
    path = startswith(shm_name, "/") ? shm_name : "/dev/shm/$(shm_name)"
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
    bright = bright_pixel_count(pixels, threshold)
    mask = gray_threshold(pixels, threshold)
    components = connected_components_count(mask, Int(frame.width), Int(frame.height))

    detections = []
    if components > 0
        push!(detections, Dict(
            "class_id" => 1,
            "label" => "defect",
            "score" => 0.85,
            "bbox" => Dict(
                "x" => frame.width * 0.25,
                "y" => frame.height * 0.25,
                "width" => frame.width * 0.5,
                "height" => frame.height * 0.5,
            ),
        ))
    end

    return Dict(
        "task_id" => req.task_id,
        "status" => "ok",
        "message" => "vision-2d julia",
        "detections" => detections,
        "metrics" => [
            Dict("name" => "bright_pixels", "value" => bright, "unit" => "count"),
            Dict("name" => "components", "value" => components, "unit" => "count"),
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
