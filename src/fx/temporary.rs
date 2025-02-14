use std::time::Duration;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crate::CellIterator;
use crate::effect::{Effect, CellFilter, IntoEffect};
use crate::effect_timer::EffectTimer;
use crate::interpolation::Interpolation::Linear;
use crate::shader::Shader;

#[derive(Clone)]
pub struct TemporaryEffect {
    effect: Effect,
    duration: EffectTimer,
}

impl TemporaryEffect {
    pub fn new(effect: Effect, duration: Duration) -> Self {
        Self { effect, duration: EffectTimer::new(duration, Linear) }
    }
}

impl Shader for TemporaryEffect {
    fn process(
        &mut self,
        duration: Duration,
        buf: &mut Buffer,
        area: Rect
    ) -> Option<Duration> {
        let remaining = self.duration.process(duration);
        let effect_area = self.effect.area().unwrap_or(area);
        self.effect.process(duration, buf, effect_area);
        remaining
    }

    fn execute(&mut self, _alpha: f32, _area: Rect, _cell_iter: CellIterator) {
        // nothing to do
    }

    fn done(&self) -> bool {
        self.duration.done() || self.effect.done()
    }

    fn clone_box(&self) -> Box<dyn Shader> {
        Box::new(self.clone())
    }

    fn area(&self) -> Option<Rect> {
        self.effect.area()
    }

    fn set_area(&mut self, area: Rect) {
        self.effect.set_area(area)
    }

    fn set_cell_selection(&mut self, strategy: CellFilter) {
        self.effect.set_cell_selection(strategy);
    }

    fn reverse(&mut self) {
        self.effect.reverse()
    }

    fn timer_mut(&mut self) -> Option<&mut EffectTimer> {
        Some(&mut self.duration)
    }

    fn cell_selection(&self) -> Option<CellFilter> {
        self.effect.cell_selection()
    }
}

pub trait IntoTemporaryEffect {
    fn with_duration(self, duration: Duration) -> Effect;
}

impl IntoTemporaryEffect for Effect {
    fn with_duration(self, duration: Duration) -> Effect {
        TemporaryEffect::new(self, duration).into_effect()
    }
}
