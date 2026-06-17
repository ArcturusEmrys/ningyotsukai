/*
    Copyright © 2020, Inochi2D Project
    Distributed under the 2-Clause BSD License, see LICENSE file.

    Authors: Luna Nielsen
*/
#version 440
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require
#extension GL_EXT_nonuniform_qualifier : require

#include "viewport.glsl"

layout(location = 0) in vec2 texUVs;
layout(location = 1) flat in uint gl_InstanceIndex;

layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outEmissive;
layout(location = 2) out vec4 outBump;

layout(set = 1, binding = 0) uniform texture2D tex[16];
layout(set = 1, binding = 1) uniform sampler samp;

struct Input {
    float opacity;
    vec3 multColor;
    vec3 screenColor;
    float emissionStrength;
    uint tex_albedo;
    uint tex_emissive;
    uint tex_bumpmap;
};

layout(set = 1, binding = 2) readonly buffer InputArray {
    Input inputs[];
} uni_in_array;

layout(set = 3, binding = 0) readonly buffer Viewports {
    Viewport viewports[];
} viewports_in;

void main() {
    Input uni_in = uni_in_array.inputs[gl_InstanceIndex];
    
    if (my_ViewIndex < viewports_in.viewports.length()) {
        vec2 scissor_origin_tl = viewports_in.viewports[my_ViewIndex].scissor_origin_tl;
        vec2 scissor_origin_br = viewports_in.viewports[my_ViewIndex].scissor_origin_br;
        if (gl_FragCoord.x < scissor_origin_tl.x || scissor_origin_br.x < gl_FragCoord.x) {
            discard;
        }

        if (gl_FragCoord.y < scissor_origin_tl.y || scissor_origin_br.y < gl_FragCoord.y) {
            discard;
        }
    }

    // Sample texture
    vec4 texColor = texture(sampler2D(tex[uni_in.tex_albedo], samp), texUVs.xy);

    // Screen color math
    vec3 screenOut = vec3(1.0) - ((vec3(1.0) - (texColor.xyz)) *
                                  (vec3(1.0) - (uni_in.screenColor * texColor.a)));

    // Multiply color math + opacity application.
    outAlbedo =
        vec4(screenOut.xyz, texColor.a) * vec4(uni_in.multColor.xyz, 1) * uni_in.opacity;

    // Emissive
    outEmissive =
        vec4(texture(sampler2D(tex[uni_in.tex_emissive], samp), texUVs.xy).xyz * uni_in.emissionStrength, 1) * outAlbedo.a;

    // Bumpmap
    outBump = vec4(texture(sampler2D(tex[uni_in.tex_bumpmap], samp), texUVs.xy).xyz, 1) * outAlbedo.a;
}