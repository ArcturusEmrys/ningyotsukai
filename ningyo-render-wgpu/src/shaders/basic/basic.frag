/*
    Copyright © 2020, Inochi2D Project
    Distributed under the 2-Clause BSD License, see LICENSE file.

    Authors: Luna Nielsen
*/
#version 440
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

#include "viewport.glsl"

layout(location = 0) in vec2 texUVs;

layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outEmissive;
layout(location = 2) out vec4 outBump;

layout(set = 1, binding = 0) uniform texture2D albedo_tex;
layout(set = 1, binding = 1) uniform texture2D emissive_tex;
layout(set = 1, binding = 2) uniform texture2D bumpmap_tex;
layout(set = 1, binding = 3) uniform sampler samp;

layout(set = 1, binding = 4) uniform Input {
    uniform float opacity;
    uniform vec3 multColor;
    uniform vec3 screenColor;
    uniform float emissionStrength;
} uni_in;

layout(set = 1, binding = 5) readonly buffer Viewports {
    uint active_viewports;
    Viewport viewports[];
} viewports_in;

void main() {
    if (gl_ViewIndex < viewports_in.active_viewports && gl_ViewIndex < viewports_in.viewports.length()) {
        vec2 scissor_origin_tl = viewports_in.viewports[gl_ViewIndex].scissor_origin_tl;
        vec2 scissor_origin_br = viewports_in.viewports[gl_ViewIndex].scissor_origin_br;
        if (gl_FragCoord.x < scissor_origin_tl.x && scissor_origin_br.x < gl_FragCoord.x) {
            discard;
        }

        if (gl_FragCoord.y < scissor_origin_tl.y && scissor_origin_br.y < gl_FragCoord.y) {
            discard;
        }
    }

    // Sample texture
    vec4 texColor = texture(sampler2D(albedo_tex, samp), texUVs);

    // Screen color math
    vec3 screenOut = vec3(1.0) - ((vec3(1.0) - (texColor.xyz)) *
                                  (vec3(1.0) - (uni_in.screenColor * texColor.a)));

    // Multiply color math + opacity application.
    outAlbedo =
        vec4(screenOut.xyz, texColor.a) * vec4(uni_in.multColor.xyz, 1) * uni_in.opacity;

    // Emissive
    outEmissive =
        vec4(texture(sampler2D(emissive_tex, samp), texUVs).xyz * uni_in.emissionStrength, 1) * outAlbedo.a;

    // Bumpmap
    outBump = vec4(texture(sampler2D(bumpmap_tex, samp), texUVs).xyz, 1) * outAlbedo.a;
}