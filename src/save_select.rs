//! Save Select game state (`src/save_select.rs`) for **CapiHunt**.
//!
//! Displays the **3 SRAM save slots** (`Slot 1`, `Slot 2`, `Slot 3`) along with
//! summary metadata (`Hunger`, `Capis Hunted`, `Survival Time`) so the player can
//! either start a new hunt bound to a slot or load a previously saved session.

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
    save::{NUM_SAVE_SLOTS, SaveSystem, SlotInfo},
    ui::{PAL_ALERT, PAL_GOLD, PAL_MUTED, PAL_WHITE, TextCanvas, init_palettes, show_menu_cursor},
};

/// Determines how choosing a slot on the Save Select screen behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveSelectMode {
    /// Pick one of the 3 slots to start a fresh game in.
    NewGame,
    /// Pick a populated slot (`SlotInfo::Valid`) to resume playing from SRAM.
    LoadGame,
}

/// Redraws all 3 slot rows, status summaries, and the `Back` option.
fn render_slots_text(
    canvas: &mut TextCanvas,
    saves: &SaveSystem,
    mode: SaveSelectMode,
    selected: usize,
    notice: Option<&str>,
) {
    canvas.clear();

    let heading = match mode {
        SaveSelectMode::NewGame => "SELECT SAVE SLOT (NEW)",
        SaveSelectMode::LoadGame => "LOAD SAVED HUNT",
    };
    canvas.draw_centred(24, 24, 192, PAL_GOLD, heading);

    for slot in 0..NUM_SAVE_SLOTS {
        let y = 44 + (slot as i32) * 24;
        let row_pal = if selected == slot { PAL_GOLD } else { PAL_WHITE };
        match saves.slot_info(slot) {
            SlotInfo::Empty => {
                let line = format!("SLOT {} - EMPTY", slot + 1);
                canvas.draw_left(46, y, 160, row_pal, &line);
                canvas.draw_left(54, y + 10, 152, PAL_MUTED, "NO DATA SAVED");
            }
            SlotInfo::Valid(summary) => {
                let mins = summary.seconds_survived / 60;
                let secs = summary.seconds_survived % 60;
                let line = format!("SLOT {} - {:02}:{:02}", slot + 1, mins, secs);
                let sub = format!(
                    "HNG:{}/3 CAPI:{} EAT:{}",
                    summary.hunger, summary.capis_hunted, summary.meat_eaten
                );
                canvas.draw_left(46, y, 160, row_pal, &line);
                canvas.draw_left(54, y + 10, 152, PAL_MUTED, &sub);
            }
            SlotInfo::Corrupted => {
                let line = format!("SLOT {} - CORRUPTED", slot + 1);
                canvas.draw_left(46, y, 160, PAL_ALERT, &line);
            }
        }
    }

    let back_pal = if selected == NUM_SAVE_SLOTS {
        PAL_GOLD
    } else {
        PAL_WHITE
    };
    canvas.draw_left(46, 120, 140, back_pal, "BACK TO TITLE");

    if let Some(msg) = notice {
        canvas.draw_centred(24, 131, 192, PAL_ALERT, msg);
    }
}

/// Runs the 3-slot Save Select screen until a game starts/loads or the user returns to Title.
pub fn run_save_select(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
    saves: &mut SaveSystem,
    mode: SaveSelectMode,
) -> AppState {
    init_palettes(gfx);
    audio.play_music(MusicId::Title);

    let mut bg = RegularBackground::new(
        Priority::P2,
        RegularBackgroundSize::Background32x32,
        TileFormat::FourBpp,
    );
    bg.fill_with(&backgrounds::PAUSE_BG);

    let mut selected: usize = 0;
    let mut tick: u32 = 0;
    let mut notice: Option<&'static str> = None;

    let mut text = TextCanvas::new();
    render_slots_text(&mut text, saves, mode, selected, notice);

    let total_rows = NUM_SAVE_SLOTS + 1; // 3 slots + Back

    loop {
        input.update();
        tick = tick.wrapping_add(1);
        let mut dirty = false;

        if input.is_just_pressed(Button::Up) {
            selected = if selected == 0 {
                total_rows - 1
            } else {
                selected - 1
            };
            notice = None;
            audio.play_sfx(SfxId::PlayerWalk);
            dirty = true;
        } else if input.is_just_pressed(Button::Down) {
            selected = (selected + 1) % total_rows;
            notice = None;
            audio.play_sfx(SfxId::PlayerWalk);
            dirty = true;
        }

        if input.is_just_pressed(Button::B) {
            audio.play_sfx(SfxId::PlayerWalk);
            return AppState::Title;
        }

        if input.is_just_pressed(Button::A) || input.is_just_pressed(Button::Start) {
            if selected == NUM_SAVE_SLOTS {
                audio.play_sfx(SfxId::PlayerWalk);
                return AppState::Title;
            }

            match mode {
                SaveSelectMode::NewGame => {
                    audio.play_sfx(SfxId::PlayerStartFire);
                    let seed = 0xC491_2026u32
                        .wrapping_add((selected as u32).wrapping_mul(0x9E37))
                        .wrapping_add(tick);
                    return AppState::PlayingNew {
                        slot: selected,
                        world_seed: seed,
                    };
                }
                SaveSelectMode::LoadGame => {
                    if let Some(saved_data) = saves.load_slot(selected) {
                        audio.set_options(saved_data.options);
                        audio.play_sfx(SfxId::PlayerEat);
                        return AppState::PlayingLoaded {
                            slot: selected,
                            data: saved_data,
                        };
                    } else {
                        audio.play_sfx(SfxId::PlayerHurt);
                        notice = Some("SLOT IS EMPTY!");
                        dirty = true;
                    }
                }
            }
        }

        if dirty {
            render_slots_text(&mut text, saves, mode, selected, notice);
        }

        audio.frame();

        let mut frame = gfx.frame();
        bg.show(&mut frame);
        text.show(&mut frame);

        let cy = if selected < NUM_SAVE_SLOTS {
            44 + (selected as i32) * 24
        } else {
            120
        };
        show_menu_cursor(&mut frame, 34, cy, tick);

        frame.commit();
    }
}
