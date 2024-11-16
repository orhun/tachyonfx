use std::cell::RefCell;
use std::rc::Rc;
use ratatui::buffer::Buffer;
use ratatui::layout::{Offset, Position, Positions, Rect};
use ratatui::style::{Color, Modifier, Style};

/// A trait for rendering the contents of one buffer onto another.
///
/// This trait is primarily implemented for `Rc<RefCell<Buffer>>`, allowing
/// for efficient rendering of one buffer's contents onto another at a specified offset.
/// This is useful for composing complex UI layouts or implementing effects that involve
/// rendering one buffer onto another.
///
/// # Safety
///
/// The implementation ensures that it does not write outside the bounds
/// of the provided buffer. The `offset` parameter is used to correctly
/// position the rendered content within the target buffer.
pub trait BufferRenderer {

    /// Renders the contents of this buffer onto the provided buffer.
    ///
    /// # Arguments
    ///
    /// * `offset` - The position offset at which to start rendering in the target buffer.
    /// * `buf` - The target buffer to render onto.
    fn render_buffer(&self, offset: Offset, buf: &mut Buffer);

    fn render_buffer_region(&self, src_region: Rect, offset: Offset, buf: &mut Buffer);
}

impl BufferRenderer for Rc<RefCell<Buffer>> {
    fn render_buffer(&self, offset: Offset, buf: &mut Buffer) {
        (*self.as_ref().borrow())
            .render_buffer(offset, buf);
    }

    fn render_buffer_region(&self, src_region: Rect, offset: Offset, buf: &mut Buffer) {
        (*self.as_ref().borrow())
            .render_buffer_region(src_region, offset, buf);
    }
}

#[cfg(feature = "sendable")]
impl BufferRenderer for crate::RefCount<Buffer> {
    fn render_buffer(&self, offset: Offset, buf: &mut Buffer) {
        (*self.lock().unwrap())
            .render_buffer(offset, buf);
    }

    fn render_buffer_region(&self, src_region: Rect, offset: Offset, buf: &mut Buffer) {
        (*self.lock().unwrap())
            .render_buffer_region(src_region, offset, buf);
    }
}

impl BufferRenderer for Buffer {
    fn render_buffer(&self, offset: Offset, buf: &mut Buffer) {
        blit_buffer(self, buf, offset);
    }

    fn render_buffer_region(&self, src_region: Rect, offset: Offset, buf: &mut Buffer) {
        blit_buffer_region(self, src_region, buf, offset);
    }
}

/// Copies the contents of a source buffer onto a destination buffer with a specified offset.
///
/// This function performs a "blit" operation, copying cells from the source buffer to the
/// destination buffer. It handles clipping on all edges, ensuring that only the overlapping
/// region is copied. The function also correctly handles negative offsets.
///
/// # Arguments
///
/// * `src` - The source buffer to copy from.
/// * `dst` - The destination buffer to copy into. This buffer is modified in-place.
/// * `offset` - The offset at which to place the top-left corner of the source buffer
///              relative to the destination buffer. Can be negative.
///
/// # Behavior
///
/// -Individual cells marked with `skip = true` in the source buffer are not copied,
///  leaving the destination cells unchanged.
/// - If the offset would place the entire source buffer outside the bounds of the
///   destination buffer, no copying occurs.
/// - The function clips the source buffer as necessary to fit within the destination buffer.
/// - Negative offsets are handled by adjusting the starting position in the source buffer.
pub fn blit_buffer(
    src: &Buffer,
    dst: &mut Buffer,
    offset: Offset,
) {
    blit_buffer_region(src, src.area, dst, offset);
}

/// Copies the specified region of a source buffer onto a destination buffer with a specified offset.
///
/// This function performs a "blit" operation, copying cells from the source buffer to the
/// destination buffer. It handles clipping on all edges, ensuring that only the overlapping
/// region is copied. The function also correctly handles negative offsets.
///
/// # Arguments
///
/// * `src` - The source buffer to copy from.
/// * `src_region` - The rectangular region within the source buffer to copy. This region will be
///                 automatically clipped to the source buffer's bounds.
/// * `dst` - The destination buffer to copy into. This buffer is modified in-place.
/// * `offset` - The offset at which to place the top-left corner of the source region
///              relative to the destination buffer. Can be negative.
///
/// # Behavior
///
/// - The source region is automatically clipped to the bounds of the source buffer.
/// - Individual cells marked with `skip = true` in the source buffer are not copied,
///   leaving the destination cells unchanged.
/// - If the offset would place the entire source buffer outside the bounds of the
///   destination buffer, no copying occurs.
/// - The function clips the source region as necessary to fit within the destination buffer.
/// - Negative offsets are handled by adjusting the starting position in the source buffer.
pub fn blit_buffer_region(
    src: &Buffer,
    src_region: Rect,
    dst: &mut Buffer,
    offset: Offset,
) {
    // clip source region to source buffer bounds
    let src_region = src_region.intersection(src.area);

    let clip = ClipRegion::new(src_region, *dst.area(), offset);
    if !clip.is_valid() {
        return; // zero area or out of bounds
    }

    // copy non-skipped cells from clipped source region to destination buffer
    for p in clip.normalized_positions() {
        let src_cell = &src[clip.src_pos(p)];
        if src_cell.skip {
            continue;
        }

        dst[clip.dst_pos(p)] = src_cell.clone();
    }
}

/// Converts a `Buffer` to an ANSI-encoded string representation.
///
/// This function takes a `Buffer` and converts it to a string that includes ANSI escape codes
/// for styling. The resulting string represents the content of the buffer with all styling
/// information (colors and text modifiers) preserved.
///
/// # Arguments
///
/// * `buffer` - A reference to the `Buffer` to be converted.
///
/// # Returns
///
/// A `String` containing the styled representation of the buffer's content.
pub fn render_as_ansi_string(buffer: &Buffer) -> String {
    let mut s = String::new();
    let mut style = Style::default();

    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell(Position::new(x, y)).unwrap();
            if cell.style() != style {
                s.push_str("\x1b[0m"); // reset
                s.push_str(&escape_code_of(cell.style()));
                style = cell.style();
            }
            s.push_str(cell.symbol());
        }
        s.push_str("\x1b[0m");
        s.push('\n');

        // need to reset the style at the end of each line,
        // so that the style correctly carries over to the next line
        style = Style::default();
    }
    s
}

fn escape_code_of(style: Style) -> String {
    let mut result = String::new();

    // Foreground color
    if let Some(color) = style.fg {
        if color != Color::Reset {
            result.push_str(&color_code(color, true));
        }
    }

    // Background color
    if let Some(color) = style.bg {
        if color != Color::Reset {
            result.push_str(&color_code(color, false));
        }
    }

    // Modifiers
    if style.add_modifier.contains(Modifier::BOLD) {
        result.push_str("\x1b[1m");
    }
    if style.add_modifier.contains(Modifier::DIM) {
        result.push_str("\x1b[2m");
    }
    if style.add_modifier.contains(Modifier::ITALIC) {
        result.push_str("\x1b[3m");
    }
    if style.add_modifier.contains(Modifier::UNDERLINED) {
        result.push_str("\x1b[4m");
    }
    if style.add_modifier.contains(Modifier::SLOW_BLINK) {
        result.push_str("\x1b[5m");
    }
    if style.add_modifier.contains(Modifier::RAPID_BLINK) {
        result.push_str("\x1b[6m");
    }
    if style.add_modifier.contains(Modifier::REVERSED) {
        result.push_str("\x1b[7m");
    }
    if style.add_modifier.contains(Modifier::HIDDEN) {
        result.push_str("\x1b[8m");
    }
    if style.add_modifier.contains(Modifier::CROSSED_OUT) {
        result.push_str("\x1b[9m");
    }

    result
}

fn color_code(color: Color, foreground: bool) -> String {
    let base = if foreground { 38 } else { 48 };
    match color {
        Color::Reset        => "\x1b[0m".to_string(),
        Color::Black        => format!("\x1b[{};5;0m", base),
        Color::Red          => format!("\x1b[{};5;1m", base),
        Color::Green        => format!("\x1b[{};5;2m", base),
        Color::Yellow       => format!("\x1b[{};5;3m", base),
        Color::Blue         => format!("\x1b[{};5;4m", base),
        Color::Magenta      => format!("\x1b[{};5;5m", base),
        Color::Cyan         => format!("\x1b[{};5;6m", base),
        Color::Gray         => format!("\x1b[{};5;7m", base),
        Color::DarkGray     => format!("\x1b[{};5;8m", base),
        Color::LightRed     => format!("\x1b[{};5;9m", base),
        Color::LightGreen   => format!("\x1b[{};5;10m", base),
        Color::LightYellow  => format!("\x1b[{};5;11m", base),
        Color::LightBlue    => format!("\x1b[{};5;12m", base),
        Color::LightMagenta => format!("\x1b[{};5;13m", base),
        Color::LightCyan    => format!("\x1b[{};5;14m", base),
        Color::White        => format!("\x1b[{};5;15m", base),
        Color::Indexed(i)   => format!("\x1b[{};5;{}m", base, i),
        Color::Rgb(r, g, b) => format!("\x1b[{};2;{};{};{}m", base, r, g, b),
    }
}

/// Helper struct to handle clipping calculations
struct ClipRegion {
    src: Rect,
    dst: Rect,
}

impl ClipRegion {
    fn new(
        src_region: Rect,
        dst_bounds: Rect,
        dst_offset: Offset
    ) -> Self {
        let x_offset = dst_offset.x.min(0).unsigned_abs() as u16;
        let y_offset = dst_offset.y.min(0).unsigned_abs() as u16;

        let dst = Rect::new(
            dst_offset.x.max(0) as u16,
            dst_offset.y.max(0) as u16,
            src_region.width,
            src_region.height
        );

        // adjust source and destination regions based on clipping and bounds
        let width = (dst.width - x_offset)
            .min(dst_bounds.width.saturating_sub(dst.x))
            .min(src_region.width);

        let height = (dst.height - y_offset)
            .min(dst_bounds.height.saturating_sub(dst.y))
            .min(src_region.height);

        Self {
            src: Rect::new(src_region.x + x_offset, src_region.y + y_offset, width, height),
            dst: Rect::new(dst.x, dst.y, width, height),
        }
    }

    fn is_valid(&self) -> bool {
        self.src.area() > 0
    }

    fn width(&self) -> u16 {
        self.src.width
    }

    fn height(&self) -> u16 {
        self.src.height
    }

    fn normalized_positions(&self) -> Positions {
        Rect::new(0, 0, self.width(), self.height()).positions()
    }

    fn src_pos(&self, pos: Position) -> Position {
        Position::new(self.src.x + pos.x, self.src.y + pos.y)
    }

    fn dst_pos(&self, pos: Position) -> Position {
        Position::new(self.dst.x + pos.x, self.dst.y + pos.y)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use crate::ref_count;
    use super::*;

    fn assert_buffer_to_buffer_copy(
        offset: Offset,
        expected: Buffer,
    ) {
        let aux_buffer = ref_count(Buffer::with_lines([
            "abcd",
            "efgh",
            "ijkl",
            "mnop",
        ]));

        let mut buf = Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]);

        aux_buffer.render_buffer(offset, &mut buf);

        assert_eq!(buf, expected)
    }

    #[test]
    fn test_render_offsets_in_bounds() {
        assert_buffer_to_buffer_copy(
            Offset { x: 0, y: 0 },
            Buffer::with_lines([
                "abcd. . ",
                "efgh. . ",
                "ijkl. . ",
                "mnop. . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
            ])
        );

        assert_buffer_to_buffer_copy(
            Offset { x: 4, y: 3 },
            Buffer::with_lines([
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . abcd",
                ". . efgh",
                ". . ijkl",
                ". . mnop",
                ". . . . ",
            ])
        );
    }

    #[test]
    fn test_render_offsets_out_of_bounds() {
        assert_buffer_to_buffer_copy(
            Offset { x: -1, y: -2 },
            Buffer::with_lines([
                "jkl . . ",
                "nop . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
            ])
        );
        assert_buffer_to_buffer_copy(
            Offset { x: 6, y: 6 },
            Buffer::with_lines([
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . . ",
                ". . . ab",
                ". . . ef",
            ])
        );
    }

    #[test]
    fn test_render_from_larger_aux_buffer() {
        let aux_buffer = ref_count(Buffer::with_lines([
            "AAAAAAAAAA",
            "BBBBBBBBBB",
            "CCCCCCCCCC",
            "DDDDDDDDDD",
            "EEEEEEEEEE",
            "FFFFFFFFFF",
        ]));

        let buffer = || Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]);

        // Test with no vertical offset
        let mut buf = buffer();
        aux_buffer.render_buffer(Offset::default(), &mut buf);
        assert_eq!(buf, Buffer::with_lines([
            "AAAAAAAA",
            "BBBBBBBB",
            "CCCCCCCC",
        ]));

        // Test with positive vertical offset
        let mut buf = buffer();
        aux_buffer.render_buffer(Offset { x: 0, y: 2 }, &mut buf);
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            "AAAAAAAA",
        ]));

        // Test with negative vertical offset
        let mut buf = buffer();
        aux_buffer.render_buffer(Offset { x: 0, y: -2 }, &mut buf);
        assert_eq!(buf, Buffer::with_lines([
            "CCCCCCCC",
            "DDDDDDDD",
            "EEEEEEEE",
        ]));

        // Test with both horizontal and vertical offset
        let mut buf = buffer();
        aux_buffer.render_buffer(Offset { x: 2, y: 1 }, &mut buf);
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". AAAAAA",
            ". BBBBBB",
        ]));

        // Test with out-of-bounds vertical offset
        let mut buf = buffer();
        aux_buffer.render_buffer(Offset { x: 0, y: 6 }, &mut buf);
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]));

        // Test with large negative vertical and horizontal offset
        let mut buf = buffer();
        aux_buffer.render_buffer(Offset { x: -5, y: -5 }, &mut buf);
        assert_eq!(buf, Buffer::with_lines([
            "FFFFF . ",
            ". . . . ",
            ". . . . ",
        ]));
    }

    #[test]
    fn test_blit_buffer_region() {
        let buffer = || Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]);

        let aux_buffer = Buffer::with_lines([
            "abcd",
            "efgh",
            "ijkl",
            "mnop",
        ]);

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(1, 1, 2, 2), &mut buf, Offset::default());
        assert_eq!(buf, Buffer::with_lines([
            "fg. . . ",
            "jk. . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(1, 1, 2, 2), &mut buf, Offset { x: 4, y: 2 });
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . fg. ",
            ". . jk. ",
            ". . . . ",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(1, 1, 3, 3), &mut buf, Offset { x: -1, y: -1 });
        assert_eq!(buf, Buffer::with_lines([
            "kl. . . ",
            "op. . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(2, 2, 3, 3), &mut buf, Offset::default());
        assert_eq!(buf, Buffer::with_lines([
            "kl. . . ",
            "op. . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(0, 0, 2, 2), &mut buf, Offset { x: 6, y: 3 });
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . ab",
            ". . . ef",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(0, 0, 2, 2), &mut buf, Offset { x: 8, y: 8 });
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(1, 1, 0, 0), &mut buf, Offset::default());
        assert_eq!(buf, Buffer::with_lines([
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
            ". . . . ",
        ]));

        let mut buf = buffer();
        blit_buffer_region(&aux_buffer, Rect::new(0, 0, 4, 4), &mut buf, Offset::default());
        assert_eq!(buf, Buffer::with_lines([
            "abcd. . ",
            "efgh. . ",
            "ijkl. . ",
            "mnop. . ",
            ". . . . ",
        ]));
    }
}