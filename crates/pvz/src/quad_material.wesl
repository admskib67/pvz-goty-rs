// Textured quad tinted by its vertex color: the mesh path of `graphics`.
import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;

@if(TONEMAP_IN_SHADER)
import bevy_core_pipeline::tonemapping;
@if(SRGB_OUTPUT)
import bevy_render::color_operations::linear_to_srgb;
@if(OKLAB_OUTPUT)
import bevy_render::color_operations::linear_rgb_to_oklab;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var texture_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var output_color = textureSample(texture, texture_sampler, in.uv);
@if(VERTEX_COLORS)
    output_color = output_color * in.color;

@if(TONEMAP_IN_SHADER)
    output_color = tonemapping::tone_mapping(output_color, bevy_sprite_render::mesh2d::view_bindings::view.color_grading);
@if(SRGB_OUTPUT)
    output_color = vec4(linear_to_srgb(output_color.rgb), output_color.a);
@if(OKLAB_OUTPUT)
    output_color = vec4(linear_rgb_to_oklab(output_color.rgb), output_color.a);
    return output_color;
}
