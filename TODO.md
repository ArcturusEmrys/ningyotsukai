 * Render to texture
   * Display texture to window for chroma keying
   * Spout2 and PipeWire for Windows and Linux
     * PipeWire on Linux is great, but OBS only uses MemPtr/SHM! It does not expose DMA-BUF!
   * ...OpenGL will fucking croak if you try this.
     * So let's use our WGPU renderer!
       * ...but GTK doesn't let you draw to surfaces
         * ~~We can draw textures but we need to ferry them from a headless WGPU renderer~~
           * ...not on Windows! Or Apple! (See the note about zero-copy GPU transforms)
           * Also, at least on Linux, we can't actually be zero-copy or we can't clear the texture!!!
         * ~~our WGPU renderer is designed to draw and present to a surface, which GTK doesn't like~~
         * So we need WGPU rendering to support headless (no target surface) mode
           * ~~First we need to get rid of the hard dependency on a target~~
           * ~~We'll have an architecture of two renderers~~
           * ~~One to feed StageRendererWidget, one to render the stage texture~~
           * They need to share as many resources as possible
           * ~~Ideally they should live in separate threads~~
           * ~~We need Arc ownership of basically everything~~
         * Figure out if we can make use of that GTK graphics offload thing
         * ~~Ideally we'd have some kind of GTK widget that provides a configured WGPU adapter~~
         * We need a low-level framework for zero-copy GPU memory transforms
           * ~~Use DMABUF on Linux, that can be pumped to GDK AND PipeWire~~
           * ~~Windows will require something different, probably DX12 shaped~~
             * I wound up just making wgpu use DX12 on Windows. Vulkan texture export is broken on AMD.
           * Apple platforms want to use IOSurface to move memory between APIs