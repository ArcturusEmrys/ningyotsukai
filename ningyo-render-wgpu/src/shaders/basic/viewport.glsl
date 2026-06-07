#extension GL_EXT_spirv_intrinsics : require

// shaderc emits the wrong definition of gl_ViewIndex if I don't do this.
spirv_decorate(extensions = ["SPV_KHR_multiview"], capabilities = [4439], 11, 4440) uint my_ViewIndex;

struct Viewport {
  mat4 projection;
  vec2 scissor_origin_tl;
  vec2 scissor_origin_br;
};