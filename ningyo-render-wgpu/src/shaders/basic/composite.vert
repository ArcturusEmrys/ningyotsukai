/*
    Copyright © 2020, Inochi2D Project
    Distributed under the 2-Clause BSD License, see LICENSE file.

    Authors: Luna Nielsen
*/
#version 440
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

#include "viewport.glsl"

layout(set = 0, binding = 0) uniform Input {
  mat4 mvp;
} uni_in;

layout(set = 0, binding = 1) readonly buffer Viewports {
  Viewport viewports[];
} viewports_in;

layout(location = 0) in vec2 verts;
layout(location = 1) in vec2 uvs;

layout(location = 0) out vec2 texUVs;

void main() {
  mat4 viewport_proj = mat4(1.0);
  if (my_ViewIndex < viewports_in.viewports.length()) {
    viewport_proj = viewports_in.viewports[my_ViewIndex].projection;
  }

  gl_Position = viewport_proj * vec4(verts, 0, 1);
  texUVs = uvs; //TODO: If the GBuffer also is a render array then we need to resize the UVs there, too.
}