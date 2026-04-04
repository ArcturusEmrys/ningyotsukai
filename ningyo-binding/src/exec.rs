//! Lua expression execution

use mlua::{Error as LuaError, Lua, LuaOptions, StdLib};
use std::sync::{Arc, RwLock, Weak};

use crate::tracker::TrackerPacket;

/// An expression evaluator for Inochi expressions.
#[derive(Clone)]
pub struct ExpressionEval(Arc<RwLock<ExpressionEvalImp>>);
pub struct ExpressionEvalImp {
    lua: Lua,
    tracker_packet: Option<TrackerPacket>,
}

impl ExpressionEval {
    pub fn new() -> Result<Self, LuaError> {
        let lua = Lua::new_with(StdLib::NONE, LuaOptions::new())?;

        let self_eval = ExpressionEval(Arc::new(RwLock::new(ExpressionEvalImp {
            lua: lua.clone(),
            tracker_packet: None,
        })));

        lua.globals().set(
            "BLEND".to_string(),
            lua.create_function({
                let callback_self = self_eval.downgrade();
                move |_, key: String| {
                    if let Some(callback_self) = callback_self.upgrade() {
                        if let Some(packet) = &callback_self.0.read().unwrap().tracker_packet {
                            if packet.facefound() {
                                return Ok(packet.value(key.as_str(), "Blendshape"));
                            }
                        }

                        //TODO: What does Inochi use as a placeholder value?
                        return Ok(None);
                    }

                    Err(LuaError::CallbackDestructed)
                }
            })?,
        )?;

        Ok(self_eval)
    }

    pub fn with_tracker_packet<T, F: FnOnce(&TrackerPacket) -> Option<T>>(
        &self,
        f: F,
    ) -> Option<T> {
        if let Some(tracker_packet) = &self.0.read().unwrap().tracker_packet {
            f(&tracker_packet)
        } else {
            None
        }
    }

    pub fn set_tracker_packet(&self, packet: TrackerPacket) {
        self.0.write().unwrap().tracker_packet = Some(packet);
    }

    pub fn eval(&self, expr: String) -> Result<f64, LuaError> {
        let me = self.0.read().unwrap();

        me.lua.load(expr).eval()
    }

    fn downgrade(&self) -> ExpressionEvalWeak {
        ExpressionEvalWeak(Arc::downgrade(&self.0))
    }
}

struct ExpressionEvalWeak(Weak<RwLock<ExpressionEvalImp>>);

impl ExpressionEvalWeak {
    fn upgrade(&self) -> Option<ExpressionEval> {
        Some(ExpressionEval(self.0.upgrade()?))
    }
}
