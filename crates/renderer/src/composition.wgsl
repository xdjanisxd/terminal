@group(0) @binding(0) var image: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let points = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(points[index], 0.0, 1.0);
}

fn encode_srgb(value: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(value, vec3(1.0 / 2.4)) - 0.055,
                  12.92 * value, value <= vec3(0.0031308));
}
fn decode_srgb(value: vec3<f32>) -> vec3<f32> {
    return select(pow((value + 0.055) / 1.055, vec3(2.4)),
                  value / 12.92, value <= vec3(0.04045));
}

// Hardware blending uses linear premultiplied RGB. Native compositors consume
// premultiplied stored RGB; convert AFTER all glyph and decoration blending.
@fragment
fn srgb_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let color = textureLoad(image, vec2<i32>(position.xy), 0);
    if color.a == 0.0 { return vec4(0.0); }
    let stored = encode_srgb(clamp(color.rgb / color.a, vec3(0.0), vec3(1.0))) * color.a;
    return vec4(decode_srgb(stored), color.a);
}

@fragment
fn linear_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(image, vec2<i32>(position.xy), 0);
}
