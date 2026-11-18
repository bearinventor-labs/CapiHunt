//! Title screen state (`src/title.rs`) for **CapiHunt**.
//!
//! Implements the attributes specified in `docs/INTENT.md`:
//! - **New Start**: proceeds to Save Select to pick a slot and begin hunting
//! - **Load Save**: proceeds to Save Select to load an existing SRAM save
//! - **Options**: toggles XM Music and WAV Sound Effects ON/OFF
//! - **Credits**: opens the scrolling Credits state

use agb::{
    display::{
        Graphics, Priority,
        object::Object,
        tiled::{RegularBackground, RegularBackgroundSize, TileFormat},
    },
    fixnum::vec2,
    input::{Button, ButtonController},
};

use crate::{
    AppState, SaveSelectMode,
    assets::{backgrounds, sprites},
    audio::{AudioManager, MusicId, SfxId},
    save::SaveSystem,
    ui::{PAL_GOLD, PAL_MUTED, PAL_WHITE, TextCanvas, init_palettes, show_menu_cursor},
};

/// Sub-screen active on the Title screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleView {
    /// Primary 4-item menu (`New Start`, `Load Save`, `Options`, `Credits`).
    Main,
    /// Inline audio options menu (`Music`, `Sound`, `Back`).
    Options,
}

/// Redraws the Title screen's foreground [`TextCanvas`] whenever selection or options change.
fn render_title_text(
    canvas: &mut TextCanvas,
    view: TitleView,
    selected: usize,
    has_saves: bool,
    music_on: bool,
    sound_on: bool,
) {
    canvas.clear();
    canvas.draw_centred(0, 18, 240, PAL_GOLD, "CAPI HUNT");
    canvas.draw_centred(0, 31, 240, PAL_MUTED, "SURVIVE THE MEADOWS");

    match view {
        TitleView::Main => {
            let items = ["NEW START", "LOAD SAVE", "OPTIONS", "CREDITS"];
            for (idx, label) in items.iter().enumerate() {
                let pal = if idx == selected {
                    PAL_GOLD
                } else if idx == 1 && !has_saves {
                    PAL_MUTED
                } else {
                    PAL_WHITE
                };
                canvas.draw_left(80, 76 + (idx as i32) * 16, 120, pal, label);
            }
        }
        TitleView::Options => {
            let m_txt = if music_on { "MUSIC: ON" } else { "MUSIC: OFF" };
            let s_txt = if sound_on { "SOUND: ON" } else { "SOUND: OFF" };
            let items = [m_txt, s_txt, "BACK"];
            for (idx, label) in items.iter().enumerate() {
                let pal = if idx == selected { PAL_GOLD } else { PAL_WHITE };
                canvas.draw_left(78, 82 + (idx as i32) * 18, 120, pal, label);
            }
        }
    }
}

/// Runs the interactive Title screen loop until the player transitions to another [`AppState`].
pub fn run_title(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
    saves: &SaveSystem,
) -> AppState {
    init_palettes(gfx);
    audio.play_music(MusicId::Title);

    let mut bg = RegularBackground::new(
        Priority::P2,
        RegularBackgroundSize::Background32x32,
        TileFormat::FourBpp,
    );
    bg.fill_with(&backgrounds::TITLE_BANNER);

    let has_saves = saves.has_any_valid_save();
    let mut view = TitleView::Main;
    let mut selected: usize = 0;
    let mut tick: u32 = 0;

    let mut text = TextCanvas::new();
    render_title_text(
        &mut text,
        view,
        selected,
        has_saves,
        audio.options().music_on,
        audio.options().sound_on,
    );

    loop {
        input.update();
        tick = tick.wrapping_add(1);
        let mut dirty = false;

        let item_count = match view {
            TitleView::Main => 4,
            TitleView::Options => 3,
        };

        if input.is_just_pressed(Button::Up) {
            selected = if selected == 0 {
                item_count - 1
            } else {
                selected - 1
            };
            audio.play_sfx(SfxId::PlayerWalk);
            dirty = true;
        } else if input.is_just_pressed(Button::Down) {
            selected = (selected + 1) % item_count;
            audio.play_sfx(SfxId::PlayerWalk);
            dirty = true;
        }

        match view {
            TitleView::Main => {
                if input.is_just_pressed(Button::A) || input.is_just_pressed(Button::Start) {
                    audio.play_sfx(SfxId::PlayerSwingBlade);
                    match selected {
                        0 => return AppState::SaveSelect(SaveSelectMode::NewGame),
                        1 => return AppState::SaveSelect(SaveSelectMode::LoadGame),
                        2 => {
                            view = TitleView::Options;
                            selected = 0;
                            dirty = true;
                        }
                        _ => return AppState::Credits,
                    }
                }
            }
            TitleView::Options => {
                if input.is_just_pressed(Button::B) {
                    view = TitleView::Main;
                    selected = 2;
                    audio.play_sfx(SfxId::PlayerWalk);
                    dirty = true;
                } else if input.is_just_pressed(Button::A)
                    || input.is_just_pressed(Button::Left)
                    || input.is_just_pressed(Button::Right)
                {
                    match selected {
                        0 => {
                            audio.toggle_music();
                            audio.play_sfx(SfxId::PlayerWalk);
                            dirty = true;
                        }
                        1 => {
                            audio.toggle_sound();
                            audio.play_sfx(SfxId::PlayerWalk);
                            dirty = true;
                        }
                        _ => {
                            if input.is_just_pressed(Button::A) {
                                view = TitleView::Main;
                                selected = 2;
                                audio.play_sfx(SfxId::PlayerWalk);
                                dirty = true;
                            }
                        }
                    }
                }
            }
        }

        if dirty {
            render_title_text(
                &mut text,
                view,
                selected,
                has_saves,
                audio.options().music_on,
                audio.options().sound_on,
            );
        }

        audio.frame();

        let mut frame = gfx.frame();
        bg.show(&mut frame);
        text.show(&mut frame);

        // Animated vignette sprites flanking the menu box (hunter, campfire, and peaceful Capi)
        let f4 = ((tick as usize) / 12) % 4;
        let mut hunter = Object::new(sprites::IDLE_RIGHT.sprite(f4));
        hunter
            .set_pos(vec2(26, 120))
            .set_priority(Priority::P1)
            .show(&mut frame);

        let mut fire = Object::new(sprites::FIRE_LIT.sprite(((tick as usize) / 6) % 4));
        fire.set_pos(vec2(38, 128))
            .set_priority(Priority::P1)
            .show(&mut frame);

        let mut capi = Object::new(sprites::CAPI_IDLE.sprite(((tick as usize) / 16) % 4));
        capi.set_pos(vec2(198, 128))
            .set_hflip(true)
            .set_priority(Priority::P1)
            .show(&mut frame);

        let (cx, cy) = match view {
            TitleView::Main => (68, 76 + (selected as i32) * 16),
            TitleView::Options => (66, 82 + (selected as i32) * 18),
        };
        show_menu_cursor(&mut frame, cx, cy, tick);

        frame.commit();
    }
}
