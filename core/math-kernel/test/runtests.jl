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

    @testset "gaussian blur lowers noise" begin
        w, h = 4, 4
        raw = UInt8[0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        blurred = gaussian_blur_3x3(raw, w, h)
        @test blurred[2] < 255
        @test blurred[2] > 0
    end
end
