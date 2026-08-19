// MONOTERMINAL Text Rendering Shader
// Target: 60 FPS (16.67ms frame budget)
// GPU Budget: 8ms render time (per SRS §2.1.1)
//
// Renders glyphs from a 4096×4096 R8Unorm texture atlas
// Single-pass rendering with per-cell foreground + background colors
// Supports ANSI color sequences, inverse video, and custom backgrounds

// Vertex input structure
struct VertexInput {
    @location(0) position: vec2<f32>,    // Screen-space position (NDC: -1 to +1)
    @location(1) tex_coord: vec2<f32>,   // Atlas UV coordinates (0.0 to 1.0)
    @location(2) fg_color: vec4<f32>,    // Foreground color (text, RGBA 0.0-1.0)
    @location(3) bg_color: vec4<f32>,    // Background color (cell, RGBA 0.0-1.0)
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,  // Required by wgpu (clip space)
    @location(0) tex_coord: vec2<f32>,            // Pass-through to fragment shader
    @location(1) fg_color: vec4<f32>,             // Pass-through to fragment shader
    @location(2) bg_color: vec4<f32>,             // Pass-through to fragment shader
}

// Texture atlas (R8Unorm = grayscale, stored in red channel)
@group(0) @binding(0)
var glyph_atlas: texture_2d<f32>;

// Atlas sampler (linear filter for antialiasing across DPI settings)
@group(0) @binding(1)
var atlas_sampler: sampler;

// Vertex shader: transform to clip space, pass through tex_coord and colors
@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Convert screen-space position to clip space (add Z=0, W=1 for 2D)
    out.clip_position = vec4<f32>(vertex.position, 0.0, 1.0);

    // Pass through texture coordinates and colors unchanged
    out.tex_coord = vertex.tex_coord;
    out.fg_color = vertex.fg_color;
    out.bg_color = vertex.bg_color;

    return out;
}

// Fragment shader: blend foreground and background based on glyph coverage
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph atlas (R8 format → alpha/coverage channel)
    // The atlas stores grayscale glyph shapes: 0.0 = no glyph, 1.0 = solid glyph
    let glyph_alpha = textureSample(glyph_atlas, atlas_sampler, in.tex_coord).r;

    // Blend background and foreground colors based on glyph coverage
    // glyph_alpha = 0.0 → bg_color (empty cell area)
    // glyph_alpha = 1.0 → fg_color (solid glyph pixel)
    // glyph_alpha = 0.5 → blend (antialiasing edge)
    // This is the industry-standard SDF/atlas text rendering approach
    return mix(in.bg_color, in.fg_color, glyph_alpha);
}
