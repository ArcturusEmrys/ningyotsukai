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

layout(location = 0) out vec4 outColor;
layout(location = 1) out vec4 outEmissive;
layout(location = 2) out vec4 outBump;

layout(set = 1, binding = 0) uniform texture2D tex[16]; //TODO: after I get bindless textures working, try removing this?
layout(set = 1, binding = 1) uniform sampler samp;

struct Input {
    float threshold;
    uint tex_albedo;
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

    vec4 color = texture(sampler2D(tex[uni_in.tex_albedo], samp), texUVs);
    if (color.a <= uni_in.threshold)
        discard;
    outColor = vec4(1, 1, 1, 1);

    //We do not touch outEmissive or outBump; they exist here solely for render
    //pass batching compatibility with the other fragment shaders.
}