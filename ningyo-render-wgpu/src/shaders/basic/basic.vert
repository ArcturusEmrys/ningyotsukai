/*
    Copyright © 2020, Inochi2D Project
    Distributed under the 2-Clause BSD License, see LICENSE file.

    Authors: Luna Nielsen
*/
#version 440
#extension GL_EXT_multiview : require

layout(set = 0, binding = 0) uniform Input {
  mat4 mvp;
  vec2 offset;
} uni_in;

struct Viewport {
  vec2 internal_size;
};

layout(set = 0, binding = 1) buffer Viewports {
  uint active_viewports;
  Viewport viewports[];
} viewports_in;

layout(location = 0) in vec2 verts;
layout(location = 1) in vec2 uvs;
layout(location = 2) in vec2 deform;

layout(location = 0) out vec2 texUVs;

void main() {
  if (gl_ViewIndex < viewports_in.active_viewports || gl_ViewIndex == 1) {
    gl_Position = uni_in.mvp * vec4(verts - uni_in.offset + deform, 0, 1);
    texUVs = uvs;
  }
}