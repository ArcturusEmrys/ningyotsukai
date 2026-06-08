#extension GL_EXT_spirv_intrinsics : require

uint my_ViewIndex = 0;

struct Viewport {
  mat4 projection;
  vec2 scissor_origin_tl;
  vec2 scissor_origin_br;
  vec4 padding;
};