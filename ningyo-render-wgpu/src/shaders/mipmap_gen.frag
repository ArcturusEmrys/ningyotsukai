#version 440

layout(location = 0) in vec2 in_vert;

layout(set = 1, binding = 0) uniform texture2D input_tex;
layout(set = 1, binding = 1) uniform sampler samp;

layout(location = 0) out vec4 outColor;

void main() {
    // Since we're using clip-space coordinates as UVs, we have to correct our
    // UVs manually.
    vec2 in_uv_space = ((in_vert * vec2(1.0, -1.0)) + vec2(1.0, 1.0)) / 2.0;

    outColor = texture(sampler2D(input_tex, samp), in_uv_space);
}