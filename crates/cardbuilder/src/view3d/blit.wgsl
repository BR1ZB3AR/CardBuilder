// Blit the rendered 3D image (premultiplied, sRGB-encoded) into egui's render pass.
@group(0) @binding(0) var blit_tex: texture_2d<f32>;
@group(0) @binding(1) var blit_samp: sampler;

struct BOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BOut {
    var o: BOut;
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    o.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

// how hard the card's print is sharpened (0 gentle … 1 strong)
const SHARPNESS: f32 = 0.6;

fn px(p: vec2<i32>, dim: vec2<i32>) -> vec4<f32> {
    return textureLoad(blit_tex, clamp(p, vec2<i32>(0), dim - 1), 0);
}

// The picture is exactly the size of the viewport: copy it pixel for pixel, with contrast-
// adaptive sharpening (after AMD's FidelityFX CAS) to win back the crispness a card loses to
// being drawn turned and in perspective. Soft detail gets the most, hard edges little, so no
// halos; pixels next to transparency (the card's outline, its shadow) are left as they are.
fn sharpened(in: BOut) -> vec4<f32> {
    let dim = vec2<i32>(textureDimensions(blit_tex));
    let p = clamp(vec2<i32>(in.uv * vec2<f32>(dim)), vec2<i32>(0), dim - 1);
    let e = px(p, dim);
    let b = px(p + vec2<i32>(0, -1), dim);
    let d = px(p + vec2<i32>(-1, 0), dim);
    let f = px(p + vec2<i32>(1, 0), dim);
    let h = px(p + vec2<i32>(0, 1), dim);
    if (min(min(e.a, b.a), min(min(d.a, f.a), h.a)) < 0.999) {
        return e;
    }
    let mn = min(e.rgb, min(min(b.rgb, d.rgb), min(f.rgb, h.rgb)));
    let mx = max(e.rgb, max(max(b.rgb, d.rgb), max(f.rgb, h.rgb)));
    let amp = sqrt(clamp(min(mn, vec3<f32>(1.0) - mx) / max(mx, vec3<f32>(1e-4)), vec3<f32>(0.0), vec3<f32>(1.0)));
    let w = amp * (-1.0 / mix(8.0, 5.0, SHARPNESS));
    let c = (e.rgb + (b.rgb + d.rgb + f.rgb + h.rgb) * w) / (vec3<f32>(1.0) + 4.0 * w);
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

@fragment
fn fs_blit(in: BOut) -> @location(0) vec4<f32> {
    return sharpened(in);
}

@fragment
fn fs_blit_srgb_target(in: BOut) -> @location(0) vec4<f32> {
    // the target encodes to sRGB itself: hand it linear (premultiplied) values
    let c = sharpened(in);
    if c.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(srgb_to_linear(c.rgb / c.a) * c.a, c.a);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}
