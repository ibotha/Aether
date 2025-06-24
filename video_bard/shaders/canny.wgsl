@group(0) @binding(0) var samp : sampler;
@group(0) @binding(1) var<uniform> dims : vec2<i32>;

@group(1) @binding(1) var buffer_a : texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(2) var buffer_b : texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(3) var buffer_c : texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(4) var buffer_d : texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(5) var buffer_e : texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(6) var buffer_f : texture_storage_2d<rgba8unorm, read_write>;


fn greyscale(c : vec3<f32>) -> f32 {
    return 0.2126*c.r + 0.7152*c.g + 0.0722*c.b;
}

fn apply_kernel(coord: vec2<i32>, kernel: array<f32, 5>, vertical: bool, dims: vec2<i32>, src: texture_storage_2d<rgba8unorm, read_write>) -> vec2<f32> {
    var offset = vec2i(1, 0);
    if vertical {
        offset = offset.yx;
    }
    var acc = kernel[2] * textureLoad(
                    src,
                    coord
                ).r;
    var div = kernel[2];
    if (vertical && coord.y > 1) || (coord.x > 1) {
        acc += kernel[0] * textureLoad(
                    src,
                    coord - offset * 2
                ).r;
        div += kernel[0];
    }
    if (vertical && coord.y > 0) || (coord.x > 0) {
        acc += kernel[1] * textureLoad(
                    src,
                    coord - offset
                ).r;
        div += kernel[1];
    }
    if (vertical && coord.y < (dims.y - 1)) || (coord.x < (dims.x - 1)) {
        acc += kernel[3] * textureLoad(
                    src,
                    coord + offset
                ).r;
        div += kernel[3];
    }
    if (vertical && coord.y < (dims.y - 2)) || (coord.x < (dims.x - 2)) {
        acc += kernel[4] * textureLoad(
                    src,
                    coord + offset * 2
                ).r;
        div += kernel[4];
    }
    return vec2f(acc, div);
}

@compute @workgroup_size(32, 1, 1)
fn main(
    @builtin(workgroup_id) WorkGroupID : vec3u,
    @builtin(local_invocation_id) LocalInvocationID : vec3u
) {
    // Each thread in the workgroup gets a 4x4 patch of pixels to turn into a greyscale image. We can call this a "Block"
    // Each workgroup works on a 32x1 blocks, so that is 128x4 pixel strips.
    // So to find our base position in the image we need our local Id to move over 4 px, and our workgroup
    // to move us over 128 and down 4
    let base_index = vec2i(WorkGroupID.xy * vec2(128, 4) + LocalInvocationID.xy * vec2(4, 4));

    for (var r = 0; r < 4; r++) {
        for (var c = 0; c < 4; c++) {
            let load_index = vec2i((base_index + vec2i(c, r)).xy);
            if load_index.x < dims.x & load_index.y < dims.y {
                let c = textureLoad(
                    buffer_a,
                    load_index
                ).rgb;
                let luma = greyscale(c);
                textureStore(buffer_b, load_index, vec4(luma, 0.0, 0.0, 1.0));
            }
        }
    }

    workgroupBarrier();

    for (var r = 0; r < 4; r++) {
        for (var c = 0; c < 4; c++) {
            let load_index = vec2i((base_index + vec2i(c, r)).xy);
            if load_index.x < dims.x & load_index.y < dims.y {
                let c = apply_kernel(load_index, array(1, 4, 6, 4, 1), false, dims, buffer_b);
                textureStore(buffer_c, load_index, vec4(c.x / c.y, 0.0, 0.0, 1.0));
            }
        }
    }

    workgroupBarrier();

    for (var r = 0; r < 4; r++) {
        for (var c = 0; c < 4; c++) {
            let load_index = vec2i((base_index + vec2i(c, r)).xy);
            if load_index.x < dims.x & load_index.y < dims.y {
                let c = apply_kernel(load_index, array(1, 4, 6, 4, 1), true, dims, buffer_c);
                textureStore(buffer_d, load_index, vec4(c.x / c.y, 0.0, 0.0, 1.0));
            }
        }
    }

    workgroupBarrier();

    for (var r = 0; r < 4; r++) {
        for (var c = 0; c < 4; c++) {
            let load_index = vec2i((base_index + vec2i(c, r)).xy);
            if load_index.x < dims.x & load_index.y < dims.y {
                let x = apply_kernel(load_index, array(0, -1, 0, 1, 0), false, dims, buffer_d);
                let y = apply_kernel(load_index, array(0, -1, 0, 1, 0), true, dims, buffer_d);
                textureStore(buffer_e, load_index, vec4(x.x, y.x, x.x*x.x+y.x*y.x, 1.0));
            }
        }
    }

    workgroupBarrier();
}