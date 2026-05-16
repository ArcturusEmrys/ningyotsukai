#version 440

layout(location = 0) in vec2 verts;
layout(location = 0) out vec2 out_verts;

void main() {
    gl_Position = vec4(verts.x, verts.y, 0, 1);
    out_verts = verts;
}