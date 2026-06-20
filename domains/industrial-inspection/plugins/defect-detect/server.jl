#!/usr/bin/env julia
# defect-detect sidecar — industrial inspection domain plugin (plugin wire v1).
#
#   julia --project=domains/industrial-inspection/plugins/defect-detect server.jl

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
    @info "defect-detect sidecar listening" socket=socket_path
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
