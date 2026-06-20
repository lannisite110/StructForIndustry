#!/usr/bin/env julia
# vision-2d sidecar — plugin wire v1 over Unix socket.
#
# Usage:
#   julia --project=plugins/vision-2d server.jl
#   SFI_VISION_SOCKET=/tmp/vision.sock julia --project=plugins/vision-2d server.jl

include("server_impl.jl")

const DEFAULT_SOCKET = joinpath(
    get(ENV, "XDG_RUNTIME_DIR", "/tmp"),
    get(ENV, "SFI_VISION_SOCKET_NAME", "sfi-plugin-vision.sock"),
)

function main()
    socket_path = get(ENV, "SFI_VISION_SOCKET", DEFAULT_SOCKET)
    ispath(socket_path) && rm(socket_path; force=true)
    mkpath(dirname(socket_path))
    listen_socket = listen(socket_path)
    @info "vision-2d sidecar listening" socket=socket_path
    try
        while true
            sock = accept(listen_socket)
            @async handle_client(sock)
        end
    finally
        close(listen_socket)
    end
end

main()
