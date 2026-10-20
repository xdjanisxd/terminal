struct RectInput {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
};

struct GlyphInput {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) content: u32,
};

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) content: u32,
};

fn corner(index: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    return corners[index];
}

@vertex
fn rect_vertex(input: RectInput, @builtin(vertex_index) index: u32) -> Output {
    let point = corner(index);
    var output: Output;
    output.position = vec4<f32>(input.rect.xy + vec2<f32>(point.x * input.rect.z, -point.y * input.rect.w), 0.0, 1.0);
    output.color = input.color;
    output.uv = point;
    output.content = 0u;
    return output;
}

@fragment
fn rect_fragment(input: Output) -> @location(0) vec4<f32> {
    return input.color;
}

@group(0) @binding(0) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(1) var glyph_sampler: sampler;

@vertex
fn glyph_vertex(input: GlyphInput, @builtin(vertex_index) index: u32) -> Output {
    let point = corner(index);
    var output: Output;
    output.position = vec4<f32>(input.rect.xy + vec2<f32>(point.x * input.rect.z, -point.y * input.rect.w), 0.0, 1.0);
    output.color = input.color;
    output.uv = mix(input.uv.xy, input.uv.zw, point);
    output.content = input.content;
    return output;
}

@fragment
fn glyph_fragment(input: Output) -> @location(0) vec4<f32> {
    let pixel = textureSample(glyph_atlas, glyph_sampler, input.uv);
    if input.content != 0u {
        var rgb = pixel.rgb;
        if input.content == 2u {
            rgb = select(pow((rgb + 0.055) / 1.055, vec3(2.4)),
                         rgb / 12.92, rgb <= vec3(0.04045));
        }
        return vec4<f32>(rgb, input.color.a * pixel.a);
    }
    return vec4<f32>(input.color.rgb, input.color.a * pixel.r);
}

// Backgrounds replace the clear rather than accumulating alpha over it.
@fragment
fn background_fragment(input: Output) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color.rgb * input.color.a, input.color.a);
}
