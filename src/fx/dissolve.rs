use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::effect_timer::EffectTimer;
use crate::shader::Shader;
use crate::simple_rng::SimpleRng;
use crate::{CellFilter, Duration};

#[derive(Clone)]
pub struct Dissolve {
    timer: EffectTimer,
    area: Option<Rect>,
    cell_filter: CellFilter,
    lcg: SimpleRng,
}

impl Dissolve {
    pub fn new(
        lifetime: EffectTimer,
    ) -> Self {
        Self {
            timer: lifetime,
            area: None,
            cell_filter: CellFilter::All,
            lcg: SimpleRng::default(),
        }
    }
}

impl Shader for Dissolve {
    fn name(&self) -> &'static str {
        if self.timer.is_reversed() { "coalesce" } else { "dissolve" }
    }

    fn execute(&mut self, _: Duration, area: Rect, buf: &mut Buffer) {
        let alpha = self.timer.alpha();
        let cell_iter = self.cell_iter(buf, area);
        let mut lcg = self.lcg;
        cell_iter
            .filter(|_| alpha > lcg.gen_f32())
            .for_each(|(_, c)| { c.set_char(' '); });
    }

    fn done(&self) -> bool {
          self.timer.done()
     }

     fn clone_box(&self) -> Box<dyn Shader> {
          Box::new(self.clone())
     }

    fn area(&self) -> Option<Rect> {
        self.area
    }

    fn set_area(&mut self, area: Rect) {
        self.area = Some(area)
    }

    fn set_cell_selection(&mut self, strategy: CellFilter) {
        self.cell_filter = strategy
    }

    fn timer(&self) -> Option<EffectTimer> {
        Some(self.timer)
    }

    fn timer_mut(&mut self) -> Option<&mut EffectTimer> {
        Some(&mut self.timer)
    }

    fn cell_selection(&self) -> Option<CellFilter> {
        Some(self.cell_filter.clone())
    }
}