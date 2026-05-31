#extension GL_EXT_spirv_intrinsics : require

// shaderc emits the wrong definition of gl_ViewIndex if I don't do this.
uint gl_ViewIndex;

struct Viewport {
  mat4 projection;
  vec2 scissor_origin_tl;
  vec2 scissor_origin_br;
};