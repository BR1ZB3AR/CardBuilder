// The 3D card: printed sides from their textures, paper edges, card-stock finishes (matte to
// gloss lamination, linen / kraft / pearl grain), foil as polished metal, spot UV as clear
// gloss — lit by a studio key light with soft shadows, a fill and a sky / floor ambient.
// Output: sRGB-encoded, premultiplied (the blit pass puts it into egui's frame).

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    cam_pos: vec4<f32>,
    // xyz towards the key light, w strength
    key_dir: vec4<f32>,
    key_col: vec4<f32>,
    fill_dir: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    // rgb linear paper colour, a roughness
    paper: vec4<f32>,
    // x lamination gloss, y grain (0 smooth, 1 linen, 2 kraft, 3 pearl), z shadows on, w floor shadow strength
    stock: vec4<f32>,
    // x exposure, y time, z shadow texel size, w floor radius
    misc: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

@group(1) @binding(0) var tex_front: texture_2d<f32>;
@group(1) @binding(1) var tex_back: texture_2d<f32>;
@group(1) @binding(2) var mask_front: texture_2d<f32>;
@group(1) @binding(3) var mask_back: texture_2d<f32>;
@group(1) @binding(4) var samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) face: f32,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) face: f32,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.nrm = v.nrm;
    o.uv = v.uv;
    o.face = v.face;
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    return g.light_vp * vec4<f32>(v.pos, 1.0);
}

fn shadow_at(world: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.stock.z < 0.5) {
        return 1.0;
    }
    let p = g.light_vp * vec4<f32>(world + n * 0.25, 1.0);
    let ndc = p.xyz / p.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z > 1.0) {
        return 1.0;
    }
    // soft edges: 5 × 5 percentage-closer filtering
    var s = 0.0;
    let t = g.misc.z * 3.5;
    for (var i = -2; i <= 2; i = i + 1) {
        for (var j = -2; j <= 2; j = j + 1) {
            s = s + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(f32(i), f32(j)) * t, ndc.z - 0.0015);
        }
    }
    return s / 25.0;
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

// studio surroundings for reflections: a bright sky, two soft boxes, a darker floor
fn env(r: vec3<f32>) -> vec3<f32> {
    let up = clamp(r.y * 0.5 + 0.5, 0.0, 1.0);
    var c = mix(g.ground.rgb * 0.6, g.sky.rgb * 1.3, smoothstep(0.25, 0.75, up));
    let box1 = normalize(vec3<f32>(-0.6, 0.55, 0.6));
    let box2 = normalize(vec3<f32>(0.7, 0.35, 0.3));
    c = c + vec3<f32>(3.0) * smoothstep(0.93, 0.985, dot(r, box1));
    c = c + vec3<f32>(1.4) * smoothstep(0.95, 0.99, dot(r, box2));
    return c;
}

fn ggx(n: vec3<f32>, h: vec3<f32>, rough: f32) -> f32 {
    let a = max(rough * rough, 0.002);
    let a2 = a * a;
    let nh = max(dot(n, h), 0.0);
    let d = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (3.14159 * d * d);
}

fn hue(t: f32) -> vec3<f32> {
    return clamp(abs(fract(t + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

// the print under one pixel, sharp: the pixel's footprint on the texture split 2 × 2, each
// quarter sampled from a finer mip level (anisotropic). A single trilinear tap would mix in
// the next-smaller, blurrier level and soften fine text and dots.
fn print_at(t: texture_2d<f32>, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let a = dx * 0.25;
    let b = dy * 0.25;
    let gx = dx * 0.4;
    let gy = dy * 0.4;
    return 0.25 * (textureSampleGrad(t, samp, uv - a - b, gx, gy) + textureSampleGrad(t, samp, uv + a - b, gx, gy)
        + textureSampleGrad(t, samp, uv - a + b, gx, gy) + textureSampleGrad(t, samp, uv + a + b, gx, gy));
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // derivatives in uniform control flow; the samples below may branch
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    let is_back = in.face > 0.5 && in.face < 1.5;
    let is_edge = in.face > 1.5;
    var base: vec3<f32>;
    var mask: vec3<f32>;
    if (is_back) {
        base = print_at(tex_back, in.uv, dx, dy).rgb;
        mask = textureSampleGrad(mask_back, samp, in.uv, dx, dy).rgb;
    } else {
        base = print_at(tex_front, in.uv, dx, dy).rgb;
        mask = textureSampleGrad(mask_front, samp, in.uv, dx, dy).rgb;
    }
    if (is_edge) {
        base = g.paper.rgb * 0.82;
        mask = vec3<f32>(0.0);
    }
    let foil = mask.r;
    let uv_varnish = mask.g;
    let holo = mask.b;

    var n = normalize(in.nrm);
    let v = normalize(g.cam_pos.xyz - in.world);
    if (dot(n, v) < 0.0 && !is_edge) {
        n = -n;
    }
    // paper grain
    let grain = g.stock.y;
    let p = in.world.xy + in.world.zy;
    var bump = vec3<f32>(0.0);
    if (grain > 0.5 && grain < 1.5) {
        // linen: fine crossed threads
        bump = vec3<f32>(sin(in.world.x * 9.0), sin(in.world.y * 9.0), sin(in.world.z * 9.0)) * 0.035;
    } else if (grain > 1.5 && grain < 2.5) {
        // kraft: fibres and specks
        let fib = noise(p * vec2<f32>(3.0, 0.6)) - 0.5;
        bump = vec3<f32>(fib, -fib, fib) * 0.06;
        base = base * (0.94 + 0.12 * noise(p * 2.3));
    }
    n = normalize(n + bump * (1.0 - foil));

    var rough = g.paper.a;
    rough = mix(rough, 0.1, uv_varnish);
    rough = mix(rough, 0.22, foil);
    if (is_edge) {
        rough = 0.95;
    }
    let metal = foil;
    if (holo > 0.01) {
        let t = dot(n, v) * 2.5 + in.uv.x * 3.0 + in.uv.y * 1.5 + g.misc.y * 0.05;
        base = mix(base, hue(t) * 0.9 + vec3<f32>(0.15), holo);
    }

    let l = normalize(g.key_dir.xyz);
    let h = normalize(l + v);
    let ndl = max(dot(n, l), 0.0);
    let sh = shadow_at(in.world, normalize(in.nrm));
    let key = g.key_col.rgb * g.key_dir.w;
    let fill = max(dot(n, normalize(g.fill_dir.xyz)), 0.0) * g.fill_dir.w;
    let amb = mix(g.ground.rgb, g.sky.rgb, n.y * 0.5 + 0.5);

    let f0 = mix(vec3<f32>(0.04), base, metal);
    let fres = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    let diffuse = base * (1.0 - metal) * (key * ndl * sh + vec3<f32>(fill) + amb * 0.55);
    let spec = fres * ggx(n, h, rough) * ndl * sh * key * 0.25;
    let r = reflect(-v, n);
    let env_amt = (1.0 - rough) * (1.0 - rough);
    // metal (foil) mirrors the studio brightly; paper only faintly
    var refl = env(r) * fres * env_amt * mix(0.6, 2.2, metal) + base * amb * 0.35 * metal;
    // lamination / varnish: a clear coat over the print
    let coat = max(g.stock.x, uv_varnish) * (1.0 - metal);
    let fc = 0.04 + 0.96 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    refl = refl + env(r) * fc * coat * 0.5 + vec3<f32>(ggx(n, h, 0.1) * ndl * sh * fc * coat * 0.05) * key;
    // pearl shimmer
    if (grain > 2.5) {
        refl = refl + hue(dot(n, v) * 1.5 + 0.3) * 0.06 * (1.0 - metal);
    }
    let col = aces((diffuse + spec + refl) * g.misc.x);
    return vec4<f32>(to_srgb(col), 1.0);
}

// the floor only shows the card's shadow (a shadow catcher over the backdrop)
@fragment
fn fs_floor(in: VOut) -> @location(0) vec4<f32> {
    let s = shadow_at(in.world, vec3<f32>(0.0, 1.0, 0.0));
    let d = length(in.world.xz) / g.misc.w;
    let fade = 1.0 - smoothstep(0.35, 1.0, d);
    let a = (1.0 - s) * g.stock.w * fade;
    return vec4<f32>(0.0, 0.0, 0.0, a);
}

@fragment
fn fs_shadow() {}
