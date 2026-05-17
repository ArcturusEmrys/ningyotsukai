use std::cmp::PartialEq;

#[derive(Default)]
pub struct RenderPassCache<S> {
    state: Option<S>,
    render_pass: Option<wgpu::RenderPass<'static>>,
}

impl<S> RenderPassCache<S>
where
    S: PartialEq + Clone,
{
    /// Start a render pass.
    ///
    /// If this is called multiple times in a row with the same state, the same
    /// render pass is returned.
    ///
    /// State must represent all of the state used to produce the given
    /// descriptor.
    pub fn render_pass_for_state(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        state: S,
        desc: &wgpu::RenderPassDescriptor,
    ) -> &mut wgpu::RenderPass<'static> {
        if self.state == Some(state.clone()) {
            if self.render_pass.is_some() {
                return self.render_pass.as_mut().unwrap();
            }
        }
        
        self.state = None;
        self.render_pass = None;

        let pass = encoder.begin_render_pass(desc).forget_lifetime();

        self.state = Some(state);
        self.render_pass = Some(pass);

        self.render_pass.as_mut().unwrap()
    }

    /// Deliberately invalidate the render pass cache.
    /// 
    /// You must call this function before finishing your command encoder, else
    /// it will panic.
    pub fn invalidate(&mut self) {
        self.state = None;
        self.render_pass = None;
    }
}
