/*
    Copyright © 2020, Inochi2D Project
    Distributed under the 2-Clause BSD License, see LICENSE file.

    Authors: Luna Nielsen
*/
#version 440
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

#include "viewport.glsl"

layout(location = 0) in vec2 verts;
layout(location = 1) in vec2 uvs;

layout(location = 0) out vec2 texUVs;

void main() {
  // We already drew projected geometry into the compositing buffer, so we just
  // need a screen-filling quad.

  gl_Position = vec4(verts, 0, 1);
  texUVs = uvs;
}