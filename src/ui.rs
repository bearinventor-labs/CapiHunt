//! Shared UI palette initialisation, menu text rendering, and HUD helpers.
//!
//! Renders multi-colour drop-shadowed text onto a single shared `32x32` grid of
//! [`DynamicTile16`] tiles on a `Priority::P0` [`RegularBackground`]. Sharing one
//! tile grid across all draw calls prevents adjacent lines (or HUD bar fills) from
//! overwriting each other's `8x8` tiles while freeing all dynamic VRAM cleanly on
//! [`TextCanvas::clear`].

use agb::{
    display::{
        Graphics, GraphicsFrame, Palette16, Priority, Rgb, Rgb15,
        font::{AlignmentKind, Layout, LayoutSettings},
        object::Object,
        tiled::{DynamicTile16, RegularBackground, RegularBackgroundSize, TileEffect, TileFormat},
    },
    fixnum::vec2,
};
use alloc::vec::Vec;

use crate::assets::{UI_FONT, backgrounds, hud_icon, sprites};

/// Background palette slot (`0..=15`) dedicated to `TextCanvas` dynamic tiles.
const UI_TEXT_BG_PALETTE: u8 = 12;

/// Colour index `1` inside `UI_TEXT_PALETTE`: crisp cream-white primary text.
pub const PAL_WHITE: u8 = 1;
/// Colour index `2` inside `UI_TEXT_PALETTE`: warm gold selected/highlighted text.
pub const PAL_GOLD: u8 = 2;
/// Colour index `3` inside `UI_TEXT_PALETTE`: soft sage-green secondary/hint text.
pub const PAL_MUTED: u8 = 3;
/// Colour index `4` inside `UI_TEXT_PALETTE`: coral-red alert/warning text.
pub const PAL_ALERT: u8 = 4;

/// Colour index `5`: 1px drop-shadow ink behind UI glyphs.
const COL_SHADOW: u8 = 5;
/// Colour index `6`: opaque dark-slate HUD bar fill.
const COL_HUD_FILL: u8 = 6;
/// Colour index `7`: warm wood bottom border along the `16px` HUD strip.
const COL_HUD_TRIM: u8 = 7;

/// Unified 16-colour UI text & HUD bar palette loaded into BG palette slot `12`.
static UI_TEXT_PALETTE: Palette16 = {
    let mut c = [Rgb15::BLACK; 16];
    c[PAL_WHITE as usize] = Rgb::new(248, 248, 232).to_rgb15();
    c[PAL_GOLD as usize] = Rgb::new(248, 216, 64).to_rgb15();
    c[PAL_MUTED as usize] = Rgb::new(160, 208, 160).to_rgb15();
    c[PAL_ALERT as usize] = Rgb::new(248, 96, 88).to_rgb15();
    c[COL_SHADOW as usize] = Rgb::new(16, 20, 28).to_rgb15();
    c[COL_HUD_FILL as usize] = Rgb::new(32, 44, 64).to_rgb15();
    c[COL_HUD_TRIM as usize] = Rgb::new(88, 56, 32).to_rgb15();
    Palette16::new(c)
};

/// Loads the shared world/screen background palette (`0..N`) plus the unified UI text palette.
pub fn init_palettes(gfx: &mut Graphics) {
    gfx.set_background_palettes(backgrounds::PALETTES);
    gfx.set_background_palette(UI_TEXT_BG_PALETTE, &UI_TEXT_PALETTE);
}

/// Composites non-zero 4bpp nibbles from `src` onto `dst`, preserving underlying pixels where `src` is `0`.
#[inline]
fn blit_non_zero_4bpp(dst: &mut u32, src: u32) {
    if src == 0 {
        return;
    }
    let mut mask = 0u32;
    let mut shift = 0u32;
    while shift < 32 {
        if ((src >> shift) & 0xF) != 0 {
            mask |= 0xF << shift;
        }
        shift += 4;
    }
    *dst = (*dst & !mask) | (src & mask);
}

/// Reusable `Priority::P0` text & HUD overlay surface backed by a shared `32x32` [`DynamicTile16`] grid.
pub struct TextCanvas {
    /// Foreground `32x32` regular background layer (`Priority::P0`).
    bg: RegularBackground,
    /// Lazily allocated `32 * 32` dynamic tiles so multiple draw calls composite cleanly on shared tiles.
    tiles: Vec<Option<DynamicTile16>>,
}

impl TextCanvas {
    /// Allocates an empty `Priority::P0` text canvas.
    #[must_use]
    pub fn new() -> Self {
        let mut tiles = Vec::with_capacity(32 * 32);
        tiles.resize_with(32 * 32, || None);
        Self {
            bg: RegularBackground::new(
                Priority::P0,
                RegularBackgroundSize::Background32x32,
                TileFormat::FourBpp,
            ),
            tiles,
        }
    }

    /// Resets the canvas, freeing all dynamic glyph tiles back to `VRAM_MANAGER`.
    pub fn clear(&mut self) {
        for slot in &mut self.tiles {
            *slot = None;
        }
        self.bg = RegularBackground::new(
            Priority::P0,
            RegularBackgroundSize::Background32x32,
            TileFormat::FourBpp,
        );
    }

    /// Scrolls the text canvas (used for smooth vertical scrolling on the Credits screen).
    pub fn set_scroll(&mut self, x: i32, y: i32) {
        self.bg.set_scroll_pos(vec2(x, y));
    }

    /// Ensures a [`DynamicTile16`] exists at `(tx, ty)` (`0..32`) and returns a mutable reference to it.
    fn ensure_tile(&mut self, tx: usize, ty: usize, fill_colour: u8) -> &mut DynamicTile16 {
        let idx = ty * 32 + tx;
        if self.tiles[idx].is_none() {
            let tile = DynamicTile16::new().fill_with(fill_colour);
            self.bg.set_tile_dynamic16(
                (tx as i32, ty as i32),
                &tile,
                TileEffect::default().palette(UI_TEXT_BG_PALETTE),
            );
            self.tiles[idx] = Some(tile);
        }
        self.tiles[idx].as_mut().unwrap()
    }

    /// Blits a configured [`Layout`] at origin `(origin_x, origin_y)` onto the shared tile grid.
    fn render_layout(&mut self, origin_x: i32, origin_y: i32, layout: Layout) {
        for group in layout {
            let gpos = group.position();
            for (px_start, packed) in group.pixels_packed() {
                if packed == 0 {
                    continue;
                }
                let x = origin_x + gpos.x + px_start.x;
                let y = origin_y + gpos.y + px_start.y;
                if !(0..256).contains(&y) || x <= -8 || x >= 256 {
                    continue;
                }
                let ty = (y as usize) / 8;
                let y_in_tile = (y as usize) & 7;
                let x_in_tile = (x.rem_euclid(8) as u32) * 4;

                if x >= 0 {
                    let tx = (x as usize) / 8;
                    if tx < 32 {
                        let tile_left = self.ensure_tile(tx, ty, 0);
                        blit_non_zero_4bpp(&mut tile_left.data_mut()[y_in_tile], packed << x_in_tile);
                    }
                }
                if x_in_tile > 0 {
                    let next_tx_i = x.div_euclid(8) + 1;
                    if (0..32).contains(&next_tx_i) {
                        let tile_right = self.ensure_tile(next_tx_i as usize, ty, 0);
                        blit_non_zero_4bpp(
                            &mut tile_right.data_mut()[y_in_tile],
                            packed >> (32 - x_in_tile),
                        );
                    }
                }
            }
        }
    }

    /// Draws left-aligned drop-shadowed text starting at pixel `(x, y)` using `colour_idx`.
    pub fn draw_left(&mut self, x: i32, y: i32, max_width: i32, colour_idx: u8, text: &str) {
        let settings = LayoutSettings::new()
            .with_max_line_length(max_width)
            .with_alignment(AlignmentKind::Left)
            .with_palette_index(colour_idx)
            .with_drop_shadow(COL_SHADOW);
        let layout = Layout::new(text, &UI_FONT, &settings);
        self.render_layout(x, y, layout);
    }

    /// Draws horizontally centred drop-shadowed text within `[x .. x + width]` at pixel `y`.
    pub fn draw_centred(&mut self, x: i32, y: i32, width: i32, colour_idx: u8, text: &str) {
        let settings = LayoutSettings::new()
            .with_max_line_length(width)
            .with_alignment(AlignmentKind::Centre)
            .with_palette_index(colour_idx)
            .with_drop_shadow(COL_SHADOW);
        let layout = Layout::new(text, &UI_FONT, &settings);
        self.render_layout(x, y, layout);
    }

    /// Fills tile rows `[start_row .. start_row + num_rows]` with opaque dark-slate HUD tiles
    /// so subsequent HUD text composites cleanly on top without punching transparent holes.
    pub fn fill_hud_bar(&mut self, start_row: i32, num_rows: i32) {
        let trim_row_word = (COL_HUD_TRIM as u32) * 0x1111_1111;
        for ry in 0..num_rows {
            let ty = (start_row + ry) as usize;
            if ty >= 32 {
                continue;
            }
            let is_bottom = ry == num_rows - 1;
            for tx in 0..30usize {
                let tile = self.ensure_tile(tx, ty, COL_HUD_FILL);
                if is_bottom {
                    tile.data_mut()[7] = trim_row_word;
                }
            }
        }
    }

    /// Submits the text layer to `frame`.
    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.bg.show(frame);
    }
}

impl Default for TextCanvas {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders an animated menu selection cursor arrow sprite at `(x, y)`.
pub fn show_menu_cursor(frame: &mut GraphicsFrame, x: i32, y: i32, anim_tick: u32) {
    let bob = ((anim_tick / 16) & 1) as i32;
    let mut cursor = Object::new(sprites::HUD_ICONS.sprite(hud_icon::MENU_CURSOR));
    cursor
        .set_pos(vec2(x + bob, y))
        .set_priority(Priority::P0)
        .show(frame);
}
