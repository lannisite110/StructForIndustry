using Test
using SFIMathKernel

@testset "SFIMathKernel" begin
    data = UInt8[10, 20, 200, 200, 10, 10]
    mn, mx, μ, σ = gray_stats(data)
    @test mn == 10
    @test mx == 200
    @test μ ≈ 75.0

  @testset "otsu separates bright blob" begin
        px = vcat(fill(UInt8(40), 100), fill(UInt8(220), 25))
        t = otsu_threshold(px)
        @test t > 100 && t < 180
    end

    @testset "morphology removes speckle" begin
        w, h = 8, 8
        mask = falses(w * h)
        mask[1] = true # isolated speckle
        mask[28] = true
        mask[29] = true
        mask[36] = true
        mask[37] = true
        opened = morph_open_3x3(mask, w, h)
        @test count(opened) < count(mask)
    end

    @testset "blob filter by area" begin
        mask = falses(16)
        # 2x2 blob at top-left
        mask[1] = mask[2] = mask[5] = mask[6] = true
        labels = connected_components_labels(mask, 4, 4)
        blobs = blob_stats_from_labels(labels, 4, 4)
        kept = filter_blobs(blobs; min_area=4)
        @test length(kept) == 1
        @test kept[1].area == 4
    end

    @testset "edge caliper rising" begin
        w, h = 128, 64
        buf = Vector{UInt8}(undef, w * h)
        edge_x = 48
        scan_y = 32
        for y in 0:(h - 1)
            for x in 0:(w - 1)
                buf[y * w + x + 1] = x < edge_x ? 30 : 220
            end
        end
        buf[scan_y * w + edge_x + 1] = 125
        result = edge_caliper_horizontal(buf, w, h, scan_y, 0, w - 1, "rising")
        @test result !== nothing
        pos, strength = result
        @test pos > 45.0 && pos < 52.0
        @test strength > 0.0
    end

    @testset "line width horizontal" begin
        w, h = 200, 40
        buf = fill(UInt8(20), w * h)
        for y in 0:(h - 1)
            for x in 40:79
                buf[y * w + x + 1] = 200
            end
        end
        result = measure_line_width_horizontal(buf, w, h, 20, 0, w - 1)
        @test result !== nothing
        width_px, left_x, right_x, _ = result
        @test width_px > 35.0 && width_px < 45.0
        @test right_x > left_x
    end

    @testset "parabolic subpixel peak center" begin
        @test parabolic_subpixel(1.0, 3.0, 1.0) ≈ 0.0
    end
end
