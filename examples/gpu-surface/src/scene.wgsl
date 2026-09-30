// The GPU surface example's scene: a cube in space, before stars, a nebula,
// a sun and a ringed planet, all traced per pixel; and the pointer's trail,
// ink that drifts and fades in a texture drawn to from the last frame's.

struct Params {
    eye: vec4<f32>,     // xyz: the camera; w: seconds
    right: vec4<f32>,   // w: tan of half the vertical field of view
    up: vec4<f32>,      // w: this frame's seconds
    forward: vec4<f32>, // w: 1 while the pointer draws a trail
    spin0: vec4<f32>,   // the cube's rotation, by columns
    spin1: vec4<f32>,   // w: how lit up the cube is (hovered, held)
    spin2: vec4<f32>,
    size: vec4<f32>,    // pixels wide and high, scale
    stroke: vec4<f32>,  // the trail's newest stretch, xy to zw, in pixels
    planet: vec4<f32>,  // xyz: its centre; w: its radius
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var ink: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

const TAU: f32 = 6.2831853;
const SUN: vec3<f32> = vec3<f32>(0.5656854, 0.3394113, -0.7542472);
const SUN_COLOR: vec3<f32> = vec3<f32>(1.0, 0.86, 0.66);
const RING_AXIS: vec3<f32> = vec3<f32>(0.2279212, 0.9116846, 0.3418817);
const CORNER: f32 = 0.09;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // One triangle over the whole target.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yxx) * q.zyx);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x = vec3<f32>(1.0, 0.0, 0.0);
    let y = vec3<f32>(0.0, 1.0, 0.0);
    let z = vec3<f32>(0.0, 0.0, 1.0);
    return mix(
        mix(mix(hash3(i), hash3(i + x), u.x), mix(hash3(i + y), hash3(i + x + y), u.x), u.y),
        mix(mix(hash3(i + z), hash3(i + x + z), u.x), mix(hash3(i + y + z), hash3(i + x + y + z), u.x), u.y),
        u.z,
    );
}

fn fbm(start: vec3<f32>) -> f32 {
    var p = start;
    var amplitude = 0.5;
    var sum = 0.0;
    for (var i = 0; i < 5; i++) {
        sum += amplitude * noise3(p);
        p = p * 2.03 + vec3<f32>(1.7, 9.2, 4.1);
        amplitude *= 0.5;
    }
    return sum;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3333));
}

// Colours that cycle as `t` goes from 0 to 1.
fn spectrum(t: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(TAU * (t + vec3<f32>(0.0, 0.33, 0.67)));
}

// Stars in three layers, each a star to a cell of a grid the direction
// passes through. `pixel`: how wide a pixel is, in radians, so stars stay
// a pixel or two across.
fn stars(dir: vec3<f32>, pixel: f32) -> vec3<f32> {
    var color = vec3<f32>(0.0);
    for (var layer = 0; layer < 3; layer++) {
        let scale = 40.0 * pow(1.8, f32(layer));
        let p = dir * scale;
        let cell = floor(p);
        let h = hash33(cell + f32(layer) * 17.0);
        let star = cell + 0.2 + 0.6 * h;
        let d = length(p - star) / (pixel * scale);
        // Most cells have none, and most stars are faint.
        let bright = step(0.55, h.x) * pow(fract(h.x * 7.31 + h.y * 3.7), 6.0) * 3.0 / (1.0 + f32(layer));
        let twinkle = 0.75 + 0.25 * sin(params.eye.w * (1.5 + 5.0 * h.z) + h.y * 40.0);
        let tint = mix(vec3<f32>(1.0, 0.72, 0.5), vec3<f32>(0.62, 0.78, 1.0), h.z);
        color += tint * bright * twinkle * (exp(-d * d * 0.9) + 0.03 * bright / (1.0 + d * d));
    }
    return color;
}

// Violet and teal dust along a band across the sky.
fn nebula(dir: vec3<f32>) -> vec3<f32> {
    let warp = vec3<f32>(fbm(dir * 1.6), fbm(dir * 1.6 + 5.2), fbm(dir * 1.6 + 9.7));
    let n = fbm(dir * 2.4 + warp * 1.8);
    let band = exp(-pow(dot(dir, vec3<f32>(0.28, 0.93, 0.23)) * 2.0, 2.0));
    let dust = smoothstep(0.32, 0.85, n) * (0.25 + band);
    let violet = vec3<f32>(0.42, 0.10, 0.60);
    let teal = vec3<f32>(0.04, 0.38, 0.52);
    let glow = mix(violet, teal, smoothstep(0.35, 0.65, warp.x)) * dust * 0.32;
    return glow + vec3<f32>(0.003, 0.004, 0.010);
}

// Whatever is infinitely far: the sky, the sun.
fn sky(dir: vec3<f32>, pixel: f32) -> vec3<f32> {
    // Rounding takes the dot a little past 1, where the pow overflows.
    let s = clamp(dot(dir, SUN), 0.0, 1.0);
    let sun = SUN_COLOR * (pow(s, 20000.0) * 30.0 + pow(s, 400.0) * 0.3 + pow(s, 16.0) * 0.05);
    return nebula(dir) + stars(dir, pixel) + sun;
}

// The sky in the cube's panels: the nebula and the sun, stars faint.
fn sky_reflected(dir: vec3<f32>, pixel: f32) -> vec3<f32> {
    return sky(dir, pixel) - stars(dir, pixel) * 0.8 + nebula(dir) * 0.5;
}

fn sphere(ro: vec3<f32>, rd: vec3<f32>, centre: vec3<f32>, radius: f32) -> f32 {
    let oc = ro - centre;
    let b = dot(oc, rd);
    let h = b * b - dot(oc, oc) + radius * radius;
    if h < 0.0 {
        return -1.0;
    }
    return -b - sqrt(h);
}

// How much of the ring there is `r` from the planet's centre: bands, with
// a gap.
fn ring_density(r: f32) -> f32 {
    let radius = params.planet.w;
    let x = (r - radius * 1.3) / (radius * 0.95);
    if x < 0.0 || x > 1.0 {
        return 0.0;
    }
    let bands = (0.7 + 0.3 * sin(r * 2.3)) * (0.75 + 0.25 * sin(r * 9.7 + 1.0));
    let gap = smoothstep(0.02, 0.05, abs(x - 0.62));
    return bands * gap * smoothstep(0.0, 0.05, x) * smoothstep(1.0, 0.9, x) * 0.85;
}

fn ring_hit(ro: vec3<f32>, rd: vec3<f32>) -> f32 {
    let t = dot(params.planet.xyz - ro, RING_AXIS) / dot(rd, RING_AXIS);
    return select(-1.0, t, t > 0.0);
}

fn planet_color(p: vec3<f32>) -> vec3<f32> {
    let n = normalize(p - params.planet.xyz);
    let latitude = dot(n, RING_AXIS);
    let swirl = fbm(n * 3.0 + vec3<f32>(0.0, params.eye.w * 0.01, 0.0));
    let bands = 0.5 + 0.5 * sin(latitude * 22.0 + swirl * 5.0);
    let base = mix(vec3<f32>(0.55, 0.32, 0.18), vec3<f32>(0.93, 0.78, 0.55), bands);
    // The ring's shadow on it.
    var light = smoothstep(-0.05, 0.25, dot(n, SUN));
    let t = ring_hit(p, SUN);
    if t > 0.0 {
        light *= 1.0 - ring_density(length(p + SUN * t - params.planet.xyz));
    }
    let rim = pow(1.0 - abs(dot(n, normalize(params.eye.xyz - p))), 3.0);
    return base * (light * SUN_COLOR + 0.015) + vec3<f32>(0.3, 0.55, 1.0) * rim * (0.15 + light);
}

fn ring_color(p: vec3<f32>) -> vec3<f32> {
    let r = length(p - params.planet.xyz);
    let dust = mix(vec3<f32>(0.75, 0.66, 0.55), vec3<f32>(0.5, 0.45, 0.42), hash3(vec3<f32>(floor(r * 4.0))));
    // The planet's shadow on it, softened at its edge.
    let to_planet = params.planet.xyz - p;
    let off_axis = length(cross(to_planet, SUN)) / params.planet.w;
    let shadow = select(1.0, mix(0.1, 1.0, smoothstep(0.96, 1.04, off_axis)), dot(to_planet, SUN) > 0.0);
    return dust * SUN_COLOR * (0.35 + 0.65 * abs(dot(RING_AXIS, SUN))) * shadow;
}

// The cube's own space: its centre at the origin, 2 across, corners
// rounded.
fn cube_distance(p: vec3<f32>) -> f32 {
    let q = abs(p) - vec3<f32>(1.0 - CORNER);
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - CORNER;
}

// Marches from where the ray meets the cube's bounds.
fn cube_hit(ro: vec3<f32>, rd: vec3<f32>) -> f32 {
    let t0 = (-1.0 - ro) / rd;
    let t1 = (1.0 - ro) / rd;
    let near = min(t0, t1);
    let far = max(t0, t1);
    let enter = max(max(near.x, near.y), near.z);
    let exit = min(min(far.x, far.y), far.z);
    if enter > exit || exit < 0.0 {
        return -1.0;
    }
    var t = max(enter, 0.0);
    for (var i = 0; i < 48; i++) {
        let d = cube_distance(ro + rd * t);
        if d < 0.0005 {
            return t;
        }
        t += d;
        if t > exit {
            break;
        }
    }
    return -1.0;
}

fn cube_normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.0005, 0.0);
    return normalize(vec3<f32>(
        cube_distance(p + e.xyy) - cube_distance(p - e.xyy),
        cube_distance(p + e.yxy) - cube_distance(p - e.yxy),
        cube_distance(p + e.yyx) - cube_distance(p - e.yyx),
    ));
}

// Iridescent panels, with glowing Truchet circuits that pulse out from
// each face's middle, and the sky reflected in them. `p` and `nl` in the
// cube's space, `n` and `rd` in the world's; `pixel`: a pixel's width in
// radians, `distance`: how far the face is.
fn cube_color(p: vec3<f32>, nl: vec3<f32>, n: vec3<f32>, rd: vec3<f32>, pixel: f32, distance: f32) -> vec3<f32> {
    let a = abs(nl);
    var uv: vec2<f32>;
    var face: f32;
    if a.x > a.y && a.x > a.z {
        uv = p.yz;
        face = sign(nl.x);
    } else if a.y > a.z {
        uv = p.xz;
        face = sign(nl.y) * 2.0;
    } else {
        uv = p.xy;
        face = sign(nl.z) * 3.0;
    }
    let time = params.eye.w;
    let cells = 5.0;
    let g = (uv * 0.5 + 0.5) * cells;
    let cell = floor(g);
    var f = fract(g) - 0.5;
    let h = hash3(vec3<f32>(cell, face * 7.0));
    if h > 0.5 {
        f.x = -f.x;
    }
    let arc = min(abs(length(f - 0.5) - 0.5), abs(length(f + 0.5) - 0.5));
    let width = max(distance * pixel * cells, 0.015);
    let line = 1.0 - smoothstep(0.035, 0.035 + width, arc);
    let seam = smoothstep(0.0, 0.03 + width, 0.5 - max(abs(f.x), abs(f.y)));
    let pulse = pow(0.5 + 0.5 * sin(time * 2.2 - length(uv) * 5.0 + face), 6.0);
    let energy = line * (0.12 + pulse) * seam;
    let emissive = mix(
        vec3<f32>(0.1, 0.9, 1.0),
        vec3<f32>(1.0, 0.25, 0.85),
        0.5 + 0.5 * sin(face * 1.7 + time * 0.4 + uv.x * 1.5),
    );

    let facing = max(dot(n, -rd), 0.0);
    let film = spectrum(facing * 1.2 + h * 0.2 + face * 0.1);
    let brushed = 0.8 + 0.2 * noise3(vec3<f32>(uv.x * 3.0, uv.y * 90.0, face));
    let base = mix(vec3<f32>(0.02, 0.022, 0.03), film * 0.5, 0.6) * brushed * (0.25 + 0.75 * seam) * (0.85 + 0.3 * h);
    let diffuse = max(dot(n, SUN), 0.0);
    let half_way = normalize(SUN - rd);
    let specular = pow(max(dot(n, half_way), 0.0), 90.0) * 3.0;
    let fresnel = 0.04 + 0.96 * pow(1.0 - facing, 5.0);
    let reflected = sky_reflected(reflect(rd, n), pixel);
    let edge = smoothstep(0.9, 1.0, max(abs(uv.x), abs(uv.y)));
    let glow = params.spin1.w * pow(1.0 - facing, 2.0);

    return base * (diffuse * SUN_COLOR * 1.6 + vec3<f32>(0.12, 0.1, 0.18))
        + (specular * SUN_COLOR + reflected * (0.2 + 0.8 * fresnel)) * seam
        + emissive * (energy * 2.5 + edge * 0.25 + glow * 0.6);
}

// The ink: last frame's, carried along a swirling flow and fading, and the
// pointer's newest stretch drawn in.
@fragment
fn trail(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(ink));
    let uv = at.xy / dims;
    let scale = params.size.z;
    let dt = params.up.w;
    let time = params.eye.w;
    // The flow: the curl of a noise field, so it swirls without sinks.
    let q = uv * params.size.xy / scale / 110.0;
    let e = 0.01;
    let n_up = noise3(vec3<f32>(q.x, q.y + e, time * 0.2));
    let n_down = noise3(vec3<f32>(q.x, q.y - e, time * 0.2));
    let n_right = noise3(vec3<f32>(q.x + e, q.y, time * 0.2));
    let n_left = noise3(vec3<f32>(q.x - e, q.y, time * 0.2));
    let flow = vec2<f32>(n_up - n_down, n_left - n_right) / (2.0 * e) * 45.0;
    let drifted = uv - flow * dt * scale / params.size.xy;
    var color = textureSampleLevel(ink, smooth_sampler, drifted, 0.0).rgb * exp(-dt * 1.3);
    if params.forward.w > 0.5 {
        let p = uv * params.size.xy;
        let a = params.stroke.xy;
        let ab = params.stroke.zw - a;
        let along = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
        let d = length(p - a - ab * along) / scale;
        let moved = length(ab) / scale;
        let sparkle = 0.5 + noise3(vec3<f32>(p / scale / 5.0, time * 3.0));
        color += spectrum(time * 0.15) * exp(-d * d / 30.0) * clamp(moved / 6.0, 0.0, 1.0) * 0.7 * sparkle;
    }
    return vec4<f32>(min(color, vec3<f32>(3.0)), 1.0);
}

@fragment
fn scene(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let size = params.size.xy;
    let uv = at.xy / size;
    // The ink bends the view through it, as a lens would, by its slope.
    let texel = 1.0 / vec2<f32>(textureDimensions(ink));
    let here = textureSampleLevel(ink, smooth_sampler, uv, 0.0).rgb;
    let dx = luma(textureSampleLevel(ink, smooth_sampler, uv + vec2<f32>(texel.x, 0.0), 0.0).rgb)
        - luma(textureSampleLevel(ink, smooth_sampler, uv - vec2<f32>(texel.x, 0.0), 0.0).rgb);
    let dy = luma(textureSampleLevel(ink, smooth_sampler, uv + vec2<f32>(0.0, texel.y), 0.0).rgb)
        - luma(textureSampleLevel(ink, smooth_sampler, uv - vec2<f32>(0.0, texel.y), 0.0).rgb);
    let bent = at.xy + vec2<f32>(dx, dy) * 18.0 * params.size.z;

    let tan_half = params.right.w;
    let ndc = (bent - size * 0.5) / (size.y * 0.5);
    let ro = params.eye.xyz;
    let rd = normalize(params.forward.xyz + (params.right.xyz * ndc.x - params.up.xyz * ndc.y) * tan_half);
    let pixel = 2.0 * tan_half / size.y;

    // Nearest last: the sky, the planet, the cube, the ring over either.
    var color = sky(rd, pixel);
    var nearest = 1e9;
    let tp = sphere(ro, rd, params.planet.xyz, params.planet.w);
    if tp > 0.0 {
        color = planet_color(ro + rd * tp);
        nearest = tp;
    }
    let spin = mat3x3<f32>(params.spin0.xyz, params.spin1.xyz, params.spin2.xyz);
    let local_ro = ro * spin;
    let local_rd = rd * spin;
    let tc = cube_hit(local_ro, local_rd);
    if tc > 0.0 && tc < nearest {
        let p = local_ro + local_rd * tc;
        let nl = cube_normal(p);
        color = cube_color(p, nl, spin * nl, rd, pixel, tc);
        nearest = tc;
    }
    let tr = ring_hit(ro, rd);
    if tr > 0.0 && tr < nearest {
        let p = ro + rd * tr;
        color = mix(color, ring_color(p), ring_density(length(p - params.planet.xyz)));
    }

    color = min(color + here * 0.9 + vec3<f32>(pow(luma(here), 3.0) * 0.5), vec3<f32>(64.0));
    // Tone mapped (ACES, fitted), and darker towards the corners.
    color = clamp((color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
    let centred = (at.xy - size * 0.5) / size;
    color *= 1.0 - 0.6 * dot(centred, centred);
    // Dithered, so the nebula's gradients don't band.
    color += (hash3(vec3<f32>(at.xy, params.eye.w)) - 0.5) / 255.0;
    return vec4<f32>(color, 1.0);
}
