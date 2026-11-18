//! Game Over screen state (`src/game_over.rs`) for **CapiHunt**.
//!
//! Implements the attributes specified in `docs/INTENT.md`:
//! - **Overall game stats** (time survived, Capis hunted, meat cooked, meat eaten,
//!   arrows fired, campfires lit, fire burns taken)
//! - **Credits** transition
//! - **Exit** to Title screen

use agb::{
    display::{
        Graphics, Priority,
        tiled::{RegularBackground, RegularBackgroundSize, TileFormat},
    },
    input::{Button, ButtonController},
};
use alloc::format;

use crate::{
    AppState,
    assets::backgrounds,
    audio::{AudioManager, MusicId, SfxId},
    save::GameStats,
    ui::{PAL_ALERT, PAL_GOLD, PAL_MUTED, PAL_WHITE, TextCanvas, init_palettes, show_menu_cursor},
};

/// Redraws the Game Over stats summary and bottom action menu (`Credits`, `Exit`).
fn render_game_over_text(canvas: &mut TextCanvas, stats: &GameStats, selected: usize) {
    canvas.clear();

    canvas.draw_centred(24, 21, 192, PAL_ALERT, "GAME OVER");

    let secs = stats.seconds_survived();
    let time_line = format!("TIME SURVIVED: {:02}:{:02}", secs / 60, secs % 60);
    let hunt_line = format!(
        "CAPIS HUNTED: {}  ARROWS: {}",
        stats.capis_hunted, stats.arrows_fired
    );
    let cook_line = format!(
        "COOKED: {}        EATEN: {}",
        stats.meat_cooked, stats.meat_eaten
    );
    let fire_line = format!(
        "FIRES LIT: {}     BURNS: {}",
        stats.fires_lit, stats.damage_taken
    );

    canvas.draw_left(36, 40, 172, PAL_WHITE, &time_line);
    canvas.draw_left(36, 54, 172, PAL_MUTED, &hunt_line);
    canvas.draw_left(36, 68, 172, PAL_MUTED, &cook_line);
    canvas.draw_left(36, 82, 172, PAL_MUTED, &fire_line);

    let items = ["CREDITS", "EXIT TO TITLE"];
    for (idx, label) in items.iter().enumerate() {
        let pal = if idx == selected { PAL_GOLD } else { PAL_WHITE };
        canvas.draw_left(74, 108 + (idx as i32) * 16, 130, pal, label);
    }
}

/// Runs the Game Over summary screen until the player chooses `Credits` or `Exit`.
pub fn run_game_over(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
    stats: GameStats,
) -> AppState {
    init_palettes(gfx);
    audio.play_music(MusicId::GameOver);

    let mut bg = RegularBackground::new(
        Priority::P2,
        RegularBackgroundSize::Background32x32,
        TileFormat::FourBpp,
    );
    bg.fill_with(&backgrounds::PAUSE_BG);

    let mut selected: usize = 0;
    let mut tick: u32 = 0;

    let mut text = TextCanvas::new();
    render_game_over_text(&mut text, &stats, selected);

    loop {
        input.update();
        tick = tick.wrapping_add(1);

        if input.is_just_pressed(Button::Up) || input.is_just_pressed(Button::Down) {
            selected ^= 1;
            audio.play_sfx(SfxId::PlayerWalk);
            render_game_over_text(&mut text, &stats, selected);
        }

        if input.is_just_pressed(Button::B) {
            audio.play_sfx(SfxId::PlayerWalk);
            return AppState::Title;
        }

        if input.is_just_pressed(Button::A) || input.is_just_pressed(Button::Start) {
            audio.play_sfx(SfxId::PlayerSwingBlade);
            return if selected == 0 {
                AppState::Credits
            } else {
                AppState::Title
            };
        }

        audio.frame();

        let mut frame = gfx.frame();
        bg.show(&mut frame);
        text.show(&mut frame);
        show_menu_cursor(&mut frame, 62, 108 + (selected as i32) * 16, tick);
        frame.commit();
    }
}
