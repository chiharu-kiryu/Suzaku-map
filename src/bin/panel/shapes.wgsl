// Scene-pixel signed distances: smooth curves at any window/DPI scale, without MSAA.
struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) point: vec2<f32>,
    @location(3) geometry: vec4<f32>,
    @location(4) style: vec4<f32>,
};
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) point: vec2<f32>,
    @location(2) @interpolate(flat) geometry: vec4<f32>,
    @location(3) @interpolate(flat) style: vec4<f32>,
};
@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    out.point = input.point;
    out.geometry = input.geometry;
    out.style = input.style;
    return out;
}
@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let radius = input.style.x;
    let stroke = input.style.y;
    let kind = input.style.z;
    let q = abs(input.point - input.geometry.xy) - input.geometry.zw + radius;
    var distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    if kind > 1.5 {
        let delta = input.geometry.zw - input.geometry.xy;
        let p = input.point - input.geometry.xy;
        let t = clamp(dot(p, delta) / max(dot(delta, delta), 0.0001), 0.0, 1.0);
        distance = length(p - delta * t) - radius;
    } else if stroke > 0.0 {
        distance = abs(distance + stroke * 0.5) - stroke * 0.5;
    }
    let aa = max(fwidth(distance), 1.0);
    let coverage = select(clamp(0.5 - distance / aa, 0.0, 1.0), 1.0, kind < 0.5);
    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
