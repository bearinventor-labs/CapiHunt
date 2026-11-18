//! Scrolling Credits screen state (`src/credits.rs`) for **CapiHunt**.
//!
//! Implements the attributes specified in `docs/INTENT.md`:
//! - **Scroll**: automatic smooth vertical scroll with D-Pad Up/Down manual control
//! - **Exit to Title**: pressing `B`, `Start`, or `A` returns cleanly to Title

use agb::{
    display::{
        Graphics, Priority,
        object::Object,
        tiled::{RegularBackground, RegularBackgroundSize, TileFormat},
    },
    fixnum::{Num, vec2},
    input::{Button, ButtonController, Tri},
};

use crate::{
    AppState,
    assets::{backgrounds, sprites},
    audio::{AudioManager, MusicId, SfxId},
    ui::{PAL_GOLD, PAL_MUTED, PAL_WHITE, TextCanvas, init_palettes},
};

/// Single entry in the scrolling credits roll (`(palette, text)`).
const CREDITS_LINES: &[(u8, &str)] = &[
    (PAL_GOLD, "CAPI HUNT"),
    (PAL_MUTED, "GBA EDITION"),
    (PAL_WHITE, ""),
    (PAL_GOLD, "DESIGN & INTENT"),
    (PAL_WHITE, "COFFEE COMPANY"),
    (PAL_WHITE, "BEARINVENTOR-LABS"),
    (PAL_WHITE, ""),
    (PAL_GOLD, "ENGINE & AUDIO"),
    (PAL_WHITE, "BUILT IN RUST NO_STD"),
    (PAL_WHITE, "AGB & AGB_TRACKER"),
    (PAL_WHITE, ""),
    (PAL_GOLD, "STARRING"),
    (PAL_WHITE, "THE FOREST HUNTER"),
    (PAL_WHITE, "PEACEFUL CAPYBARAS"),
    (PAL_WHITE, ""),
    (PAL_MUTED, "B / START: EXIT TO TITLE"),
];

/// Runs the scrolling Credits screen until the player exits back to [`AppState::Title`].
pub fn run_credits(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
) -> AppState {
    init_palettes(gfx);
    audio.play_music(MusicId::Title);

    let mut bg = RegularBackground::new(
        Priority::P2,
        RegularBackgroundSize::Background32x32,
        TileFormat::FourBpp,
    );
    bg.fill_with(&backgrounds::CREDITS_BG);

    // Lay out all credits lines within the 256px tall 32x32 text canvas so hardware BG
    // vertical scrolling (`set_scroll`) glides smoothly at 60 FPS with zero per-frame redraws!
    let mut text = TextCanvas::new();
    for (idx, &(pal, line)) in CREDITS_LINES.iter().enumerate() {
        if !line.is_empty() {
            text.draw_centred(24, 8 + (idx as i32) * 14, 192, pal, line);
        }
    }

    // Start scrolled just below the screen so the credits rise up from the bottom!
    let mut scroll_y = Num::<i32, 8>::new(-148);
    let mut tick: u32 = 0;

    loop {
        input.update();
        tick = tick.wrapping_add(1);

        if input.is_just_pressed(Button::B)
            || input.is_just_pressed(Button::Start)
            || input.is_just_pressed(Button::A)
        {
            audio.play_sfx(SfxId::PlayerWalk);
            return AppState::Title;
        }

        // Default gentle upward drift (~0.35 px/frame), steerable with D-Pad Up/Down!
        let speed = match input.y_tri() {
            Tri::Positive => Num::<i32, 8>::new(3) / 2,
            Tri::Negative => Num::<i32, 8>::new(-1),
            Tri::Zero => Num::<i32, 8>::new(3) / 8,
        };
        scroll_y += speed;
        if scroll_y.floor() > 236 {
            scroll_y = Num::new(-152);
        }
        if scroll_y.floor() < -156 {
            scroll_y = Num::new(236);
        }

        text.set_scroll(0, scroll_y.floor());
        audio.frame();

        let mut frame = gfx.frame();
        bg.show(&mut frame);
        text.show(&mut frame);

        // Cute waddling Capi walking along the bottom meadow edge during credits!
        let capi_x = ((tick / 2) % 264) as i32 - 16;
        let mut capi = Object::new(sprites::CAPI_WALK.sprite(((tick as usize) / 8) % 4));
        capi.set_pos(vec2(capi_x, 144))
            .set_priority(Priority::P1)
            .show(&mut frame);

        frame.commit();
    }
}
