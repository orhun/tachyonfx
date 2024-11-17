use ratatui::buffer::Cell;
use ratatui::layout;
use ratatui::layout::{Margin, Position, Rect};
use ratatui::prelude::Color;
use crate::color_ext::ToRgbComponents;
use crate::{ref_count, RefCount, ThreadSafetyMarker};

#[cfg(not(feature = "sendable"))]
type CellPredFn = RefCount<dyn Fn(&Cell) -> bool>;
#[cfg(feature = "sendable")]
type CellPredFn = RefCount<dyn Fn(&Cell) -> bool + Send>;

#[cfg(not(feature = "sendable"))]
type PositionFnType = RefCount<dyn Fn(Position) -> bool>;
#[cfg(feature = "sendable")]
type PositionFnType = RefCount<dyn Fn(Position) -> bool + Send>;

/// A filter mode that enables effects to operate on specific cells based on various criteria.
///
/// `CellFilter` provides a flexible way to select cells for applying effects based on their
/// properties such as colors, position, content, or custom predicates. Filters can be combined
/// using logical operations to create complex selection patterns.
#[derive(Clone, Default)]
pub enum CellFilter {
    /// Selects every cell
    #[default]
    All,
    /// Selects cells with matching foreground color
    FgColor(Color),
    /// Selects cells with matching background color
    BgColor(Color),
    /// Selects cells within the inner margin of the area
    Inner(Margin),
    /// Selects cells outside the inner margin of the area
    Outer(Margin),
    /// Selects cells with text
    Text,
    /// Selects cells that match all the given filters
    AllOf(Vec<CellFilter>),
    /// Selects cells that match any of the given filters
    AnyOf(Vec<CellFilter>),
    /// Selects cells that do not match any of the given filters
    NoneOf(Vec<CellFilter>),
    /// Negates the given filter
    Not(Box<CellFilter>),
    /// Selects cells within the specified layout, denoted by the index
    Layout(layout::Layout, u16),
    /// Selects cells by predicate function
    PositionFn(PositionFnType),
    /// Selects cells by predicate function
    EvalCell(CellPredFn),
}

impl CellFilter {
    /// Creates a new cell filter using a custom evaluation function.
    ///
    /// The provided function should return `true` for cells that should be selected
    /// and `false` for cells that should be excluded.
    ///
    /// # Arguments
    /// * `f` - A function that takes a reference to a Cell and returns a boolean
    ///
    /// # Type Parameters
    /// * `F` - A function type that implements the required thread safety markers
    pub fn eval_cell<F>(f: F) -> Self
        where F: Fn(&Cell) -> bool + ThreadSafetyMarker + 'static
    {
        CellFilter::EvalCell(ref_count(f))
    }

    /// Converts the filter to a human-readable string representation.
    ///
    /// This method is useful for debugging and logging purposes, providing
    /// a clear visualization of the filter's structure and parameters.
    ///
    /// # Returns
    /// A String representing the filter in a readable format
    pub fn to_string(&self) -> String {
        fn to_hex(c: &Color) -> String {
            let (r, g, b) = c.to_rgb();
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        }

        fn format_margin(m: &Margin) -> String {
            format!("{}:{}", m.horizontal, m.vertical)
        }

        fn to_string(filters: &[CellFilter]) -> String {
            filters.iter()
                .map(CellFilter::to_string)
                .collect::<Vec<String>>()
                .join(", ")
        }

        match self {
            CellFilter::All             => "all".to_string(),
            CellFilter::FgColor(color)  => format!("fg({})", to_hex(color)),
            CellFilter::BgColor(color)  => format!("bg({})", to_hex(color)),
            CellFilter::Inner(m)        => format!("inner({})", format_margin(m)),
            CellFilter::Outer(m)        => format!("outer({})", format_margin(m)),
            CellFilter::Text            => "text".to_string(),
            CellFilter::AllOf(filters)  => format!("all_of({})", to_string(filters)),
            CellFilter::AnyOf(filters)  => format!("any_of({})", to_string(filters)),
            CellFilter::NoneOf(filters) => format!("none_of({})", to_string(filters)),
            CellFilter::Not(filter)     => format!("!{}", filter.to_string()),
            CellFilter::Layout(_, idx)  => format!("layout({})", idx),
            CellFilter::PositionFn(_)   => "position_fn".to_string(),
            CellFilter::EvalCell(_)     => "eval_cell".to_string(),
        }
    }
}

/// A predicate that evaluates cells based on their position and properties using a specified filter strategy.
///
/// `CellPredicate` is created internally by `CellFilter`'s `selector` method and serves as the
/// evaluation engine for cell filtering operations. It combines spatial awareness (via a rectangular area)
/// with content-based filtering rules to determine which cells should be included in operations.
///
/// See also [crate::Shader::cell_iter].
pub struct CellPredicate {
    /// The effective area for cell evaluation after applying any area-modifying filters.
    /// This may be different from the original area if the filter modifies spatial bounds
    /// (e.g., margins or layout sections).
    inner_area: Rect,

    /// The filter strategy that defines the criteria cells must meet to be considered valid.
    /// This strategy can combine multiple filters using logical operations (AND, OR, NOT)
    /// and can include both position-based and content-based criteria.
    strategy: CellFilter,
}

impl CellPredicate {
    /// Creates a new `CellPredicate` with the specified area and filter strategy.
    ///
    /// The provided area may be modified based on the filter strategy (e.g., for margin-based filters).
    ///
    /// # Arguments
    /// * `area` - The initial rectangular area for cell evaluation
    /// * `strategy` - The filter strategy to apply
    fn new(area: Rect, strategy: CellFilter) -> Self {
        let inner_area = Self::resolve_area(area, &strategy);

        Self { inner_area, strategy }
    }

    fn resolve_area(area: Rect, mode: &CellFilter) -> Rect {
        match mode {
            CellFilter::All                  => area,
            CellFilter::Inner(margin)        => area.inner(*margin),
            CellFilter::Outer(margin)        => area.inner(*margin),
            CellFilter::Text                 => area,
            CellFilter::AllOf(_)             => area,
            CellFilter::AnyOf(_)             => area,
            CellFilter::NoneOf(_)            => area,
            CellFilter::Not(m)               => Self::resolve_area(area, m.as_ref()),
            CellFilter::FgColor(_)           => area,
            CellFilter::BgColor(_)           => area,
            CellFilter::Layout(layout, idx)  => layout.split(area)[*idx as usize],
            CellFilter::PositionFn(_)        => area,
            CellFilter::EvalCell(_)          => area,
        }
    }

    /// Determines if a cell at the given position meets the filter criteria.
    ///
    /// This method combines position-based and cell-content-based filtering to make
    /// the final determination.
    ///
    /// # Arguments
    /// * `pos` - The position to evaluate
    /// * `cell` - The cell at the given position
    ///
    /// # Returns
    /// `true` if the cell meets all filter criteria, `false` otherwise
    pub fn is_valid(&self, pos: Position, cell: &Cell) -> bool {
        let mode = &self.strategy;

        self.valid_position(pos, mode)
            && self.is_valid_cell(cell, mode)
    }

    fn valid_position(&self, pos: Position, mode: &CellFilter) -> bool {
        fn apply_position_fn(f: &PositionFnType, pos: Position) -> bool {
            #[cfg(not(feature = "sendable"))]
            return f.borrow()(pos);
            #[cfg(feature = "sendable")]
            f.lock().unwrap()(pos)
        }

        match mode {
            CellFilter::All           => self.inner_area.contains(pos),
            CellFilter::Layout(_, _)  => self.inner_area.contains(pos),
            CellFilter::Inner(_)      => self.inner_area.contains(pos),
            CellFilter::Outer(_)      => !self.inner_area.contains(pos),
            CellFilter::Text          => self.inner_area.contains(pos),
            CellFilter::AllOf(s)      => s.iter()
                .all(|mode| mode.selector(self.inner_area).valid_position(pos, mode)),
            CellFilter::AnyOf(s)      => s.iter()
                .any(|mode| mode.selector(self.inner_area).valid_position(pos, mode)),
            CellFilter::NoneOf(s)     => s.iter()
                .all(|mode| !mode.selector(self.inner_area).valid_position(pos, mode)),
            CellFilter::Not(m)        => self.valid_position(pos, m.as_ref()),
            CellFilter::FgColor(_)    => self.inner_area.contains(pos),
            CellFilter::BgColor(_)    => self.inner_area.contains(pos),
            CellFilter::PositionFn(f) => apply_position_fn(f, pos),
            CellFilter::EvalCell(_)   => self.inner_area.contains(pos),
        }
    }

    fn is_valid_cell(&self, cell: &Cell, mode: &CellFilter) -> bool {
        fn apply_eval_fn(f: &CellPredFn, cell: &Cell) -> bool {
            #[cfg(not(feature = "sendable"))]
            return f.borrow()(cell);
            #[cfg(feature = "sendable")]
            f.lock().unwrap()(cell)
        }

        match mode {
            CellFilter::Text => {
                if cell.symbol().len() == 1 {
                    let ch = cell.symbol().chars().next().unwrap();
                    ch.is_alphabetic() || ch.is_numeric() || ch == ' ' || "?!.,:;".contains(ch)
                } else {
                    false
                }
            },

            CellFilter::AllOf(s) => {
                s.iter()
                    .all(|s| s.selector(self.inner_area).is_valid_cell(cell, s))
            },

            CellFilter::FgColor(color) => cell.fg == *color,
            CellFilter::BgColor(color) => cell.bg == *color,

            CellFilter::Not(m) => !self.is_valid_cell(cell, m.as_ref()),

            CellFilter::EvalCell(f) => apply_eval_fn(f, cell),

            _ => true,
        }
    }
}

impl CellFilter {
    pub fn selector(&self, area: Rect) -> CellPredicate {
        CellPredicate::new(area, self.clone())
    }
}

#[cfg(test)]
mod tests {
    use layout::Layout;
    use ratatui::buffer::Buffer;
    use crate::duration::duration::Duration;
    use crate::EffectRenderer;
    use crate::fx::effect_fn;
    use super::*;

    #[test]
    fn test_cell_filter_to_string() {
        let filter = CellFilter::FgColor(Color::Red);
        assert_eq!(filter.to_string(), "fg(#800000)");

        let filter = CellFilter::BgColor(Color::Green);
        assert_eq!(filter.to_string(), "bg(#008000)");

        let filter = CellFilter::Inner(Margin::new(1, 1));
        assert_eq!(filter.to_string(), "inner(1:1)");

        let filter = CellFilter::Outer(Margin::new(3, 4));
        assert_eq!(filter.to_string(), "outer(3:4)");

        let filter = CellFilter::Text;
        assert_eq!(filter.to_string(), "text");

        let filter = CellFilter::AllOf(vec![
            CellFilter::FgColor(Color::Red),
            CellFilter::BgColor(Color::Green),
        ]);
        assert_eq!(filter.to_string(), "all_of(fg(#800000), bg(#008000))");

        let filter = CellFilter::AnyOf(vec![
            CellFilter::FgColor(Color::Red),
            CellFilter::BgColor(Color::Green),
        ]);
        assert_eq!(filter.to_string(), "any_of(fg(#800000), bg(#008000))");

        let filter = CellFilter::NoneOf(vec![
            CellFilter::FgColor(Color::Red),
            CellFilter::BgColor(Color::Green),
        ]);
        assert_eq!(filter.to_string(), "none_of(fg(#800000), bg(#008000))");

        let filter = CellFilter::Not(Box::new(CellFilter::FgColor(Color::Red)));
        assert_eq!(filter.to_string(), "!fg(#800000)");

        let filter = CellFilter::Layout(Layout::horizontal(&[]), 0);
        assert_eq!(filter.to_string(), "layout(0)");

        let filter = CellFilter::PositionFn(ref_count(|_| true));
        assert_eq!(filter.to_string(), "position_fn");

        let filter = CellFilter::EvalCell(ref_count(|_| true));
        assert_eq!(filter.to_string(), "eval_cell");
    }

    #[test]
    fn test_cell_filter_eval() {
        let mut buf = Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]);

        let filter = CellFilter::eval_cell(|cell| cell.symbol() == ".");
        let mut fx = effect_fn((), 1, |_, _, cells| {
            for (_, c) in cells {
                c.set_symbol("X");
            }
        }).with_cell_selection(filter);

        let area = buf.area().clone();
        buf.render_effect(&mut fx, area, Duration::from_millis(16));

        assert_eq!(buf, Buffer::with_lines([
            "X X X X ",
            "X X X X ",
            "X X X X ",
            "X X X X ",
        ]));
    }
}