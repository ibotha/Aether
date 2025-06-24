@group(0) @binding(0) var samp : sampler;
@group(1) @binding(1) var input_tex : texture_2d<f32>;
@group(1) @binding(2) var output_tex : texture_storage_2d<rgba8unorm, write>;

@group(2) @binding(1) var buffer_a : texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(2) var buffer_b : texture_storage_2d<rgba8unorm, write>;

// Store uminence in the red channel of a color
fn greyscale(c : vec3<f32>) -> f32 {
    return 0.2126*c.r + 0.7152*c.g + 0.0722*c.b;
}

@compute @workgroup_size(32, 1, 1)
fn main(
    @builtin(workgroup_id) WorkGroupID : vec3u,
    @builtin(local_invocation_id) LocalInvocationID : vec3u
) {
    // Each thread in the workgroup gets a 4x4 block to turn into a greyscale image. We can call this a "Block"
    // Each workgroup works on a 32x1 blocks, so that is 128x4 pixel strips.
    // So to find our base position in the image we need our local Id to move over 4 px, and our workgroup
    // to move us over 128 and down 4
    let dims = vec2u(textureDimensions(input_tex, 0));
    let base_index = vec2i(WorkGroupID.xy * vec2(128, 4) + LocalInvocationID.xy * vec2(4, 4));

    for (var r = 0; r < 4; r++) {
        for (var c = 0; c < 4; c++) {
            let load_index = vec2u((base_index + vec2i(c, r)).xy);
            if load_index.x < dims.x & load_index.y < dims.y {
                let c = textureSampleLevel(
                    input_tex,
                    samp,
                    (vec2f(load_index) + vec2f(0.5, 0.5)) / vec2f(dims),
                    0.0
                ).rgb;
                let luma = greyscale(c);
                textureStore(output_tex, load_index, vec4(luma, luma, luma, 1.0));
            }
        }
    }
}