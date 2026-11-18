//! Centralised compile-time asset imports for **CapiHunt**.
//!
//! Every graphic, font, tracker music module, and PCM sound effect required by
//! `docs/INTENT.md` is embedded into GBA ROM here using `agb` and `agb_tracker`
//! macros so all game states can reference strongly typed static assets with
//! zero runtime file I/O.

use agb::{display::font::Font, include_aseprite, include_background_gfx, include_font, include_wav, sound::mixer::SoundData};
use agb_tracker::{Track, include_xm};

// ---------------------------------------------------------------------------
// 4bpp (15-bit colour) Sprites: Player (8x16), Capis (8x8), Fire & Items (8x8)
// ---------------------------------------------------------------------------
include_aseprite!(
    pub mod sprites,
    // Player animations: 8x16, 4 frames per strip, 3 facings (left uses hflip on right)
    8x16 "gfx/player/idle_front.png",
    8x16 "gfx/player/idle_right.png",
    8x16 "gfx/player/idle_back.png",

    8x16 "gfx/player/walk_front.png",
    8x16 "gfx/player/walk_right.png",
    8x16 "gfx/player/walk_back.png",

    8x16 "gfx/player/run_front.png",
    8x16 "gfx/player/run_right.png",
    8x16 "gfx/player/run_back.png",

    8x16 "gfx/player/shoot_bow_front.png",
    8x16 "gfx/player/shoot_bow_right.png",
    8x16 "gfx/player/shoot_bow_back.png",

    8x16 "gfx/player/swing_blade_front.png",
    8x16 "gfx/player/swing_blade_right.png",
    8x16 "gfx/player/swing_blade_back.png",

    8x16 "gfx/player/take_damage_front.png",
    8x16 "gfx/player/take_damage_right.png",
    8x16 "gfx/player/take_damage_back.png",

    8x16 "gfx/player/eat_front.png",
    8x16 "gfx/player/eat_right.png",
    8x16 "gfx/player/eat_back.png",

    8x16 "gfx/player/start_fire_front.png",
    8x16 "gfx/player/start_fire_right.png",
    8x16 "gfx/player/start_fire_back.png",

    // Capybara ("Capi") animations: 8x8, 4 frames per strip
    8x8 "gfx/capi/capi_idle.png",
    8x8 "gfx/capi/capi_walk.png",
    8x8 "gfx/capi/capi_run.png",
    8x8 "gfx/capi/capi_hurt.png",

    // Campfire states: 8x8, 4 frames per strip (unlit & flickering lit)
    8x8 "gfx/etc/fire_unlit.png",
    8x8 "gfx/etc/fire_lit.png",

    // Projectiles, melee arc gleam, pickups, and HUD icons (8x8)
    8x8 "gfx/etc/arrow_h.png",
    8x8 "gfx/etc/arrow_v.png",
    8x8 "gfx/etc/blade_slash_h.png",
    8x8 "gfx/etc/blade_slash_v.png",
    8x8 "gfx/etc/meat_raw.png",
    8x8 "gfx/etc/meat_cooked.png",
    8x8 "gfx/etc/arrow_pickup.png",
    8x8 "gfx/etc/hud_icons.png"
);

// ---------------------------------------------------------------------------
// 4bpp (15-bit colour) Tiled Backgrounds: World, Title, Pause, and Credits
// ---------------------------------------------------------------------------
include_background_gfx!(
    pub mod backgrounds,
    "182818",
    WORLD_TILES => "gfx/world/world_tiles.png",
    TITLE_BANNER => deduplicate "gfx/title/title_banner.png",
    PAUSE_BG => deduplicate "gfx/pause/pause_bg.png",
    CREDITS_BG => deduplicate "gfx/credits/credits_bg.png"
);

// ---------------------------------------------------------------------------
// Bitmap UI Font (6x8 pixel font for HUD, Menus, Save Select, Stats, Credits)
// ---------------------------------------------------------------------------
/// Crisp 6x8 pixel font used by [`agb::display::font::Layout`] across all menus and HUD text.
pub static UI_FONT: Font = include_font!("gfx/etc/font.json", "gfx/etc/font.png");

// ---------------------------------------------------------------------------
// HUD Icon Sprite Indices inside `sprites::HUD_ICONS`
// ---------------------------------------------------------------------------
/// Indices into the 8-frame `HUD_ICONS` tag (`gfx/etc/hud_icons.png`).
pub mod hud_icon {
    /// Filled hunger drumstick icon.
    pub const HUNGER_FULL: usize = 0;
    /// Empty hunger slot outline icon.
    pub const HUNGER_EMPTY: usize = 1;
    /// Hunting blade weapon icon.
    pub const WEAPON_BLADE: usize = 2;
    /// Hunting bow weapon icon.
    pub const WEAPON_BOW: usize = 3;
    /// Carried raw meat inventory icon.
    pub const MEAT_RAW: usize = 4;
    /// Carried cooked meat inventory icon.
    pub const MEAT_COOKED: usize = 5;
    /// Menu selection cursor arrow icon.
    pub const MENU_CURSOR: usize = 6;
    /// Campfire cooking progress spark icon.
    pub const COOK_SPARK: usize = 7;
}

// ---------------------------------------------------------------------------
// XM Tracker Music Modules (`gfx/music/*.xm`)
// ---------------------------------------------------------------------------
/// Embedded FastTracker II (`.xm`) music tracks played via `agb_tracker`.
pub mod music {
    use super::*;

    /// Adventurous pastoral theme for the Title and Save Select screens.
    pub static TITLE: Track = include_xm!("gfx/music/title.xm");
    /// Gentle open-meadow wilderness loop for active hunting gameplay.
    pub static PLAYING: Track = include_xm!("gfx/music/playing.xm");
    /// Calm campfire interlude loop played while paused.
    pub static PAUSE: Track = include_xm!("gfx/music/pause.xm");
    /// Sombre minor-key cadence for the Game Over screen (also used softly on Credits).
    pub static GAME_OVER: Track = include_xm!("gfx/music/game_over.xm");
}

// ---------------------------------------------------------------------------
// Mono 16-bit PCM WAV Sound Effects @ 18157 Hz (`gfx/sound/*.wav`)
// ---------------------------------------------------------------------------
/// Embedded WAV sound effects sampled at [`agb::sound::mixer::Frequency::Hz18157`].
pub mod sfx {
    use super::*;

    /// Soft grass footstep when the player walks.
    pub static PLAYER_WALK: SoundData = include_wav!("gfx/sound/player_walk.wav");
    /// Brisk double-crunch footstep when the player sprints (`Hold L`).
    pub static PLAYER_RUN: SoundData = include_wav!("gfx/sound/player_run.wav");
    /// Burn / damage sound when the player steps onto a lit campfire.
    pub static PLAYER_HURT: SoundData = include_wav!("gfx/sound/player_hurt.wav");
    /// Chomp + chime played when eating cooked meat (`Button R`).
    pub static PLAYER_EAT: SoundData = include_wav!("gfx/sound/player_eat.wav");
    /// Flint strike and ignition whoosh when lighting an unlit campfire (`Button R`).
    pub static PLAYER_START_FIRE: SoundData = include_wav!("gfx/sound/player_start_fire.wav");
    /// Bowstring twang and arrow release (`Hold A`).
    pub static PLAYER_SHOOT_BOW: SoundData = include_wav!("gfx/sound/player_shoot_bow.wav");
    /// Steel slash swoosh when swinging the hunting blade (`Button A`).
    pub static PLAYER_SWING_BLADE: SoundData = include_wav!("gfx/sound/player_swing_blade.wav");

    /// Quiet paw tap while a nearby Capi wanders.
    pub static CAPI_WALK: SoundData = include_wav!("gfx/sound/capi_walk.wav");
    /// Quick scurrying footsteps while a startled Capi flees.
    pub static CAPI_RUN: SoundData = include_wav!("gfx/sound/capi_run.wav");
    /// High-pitched squeak when a Capi takes a hit from blade or arrow.
    pub static CAPI_HURT: SoundData = include_wav!("gfx/sound/capi_hurt.wav");

    /// Ambient breeze swell across the meadows.
    pub static WORLD_WIND: SoundData = include_wav!("gfx/sound/world_wind.wav");
    /// Ambient forest foliage rustle.
    pub static WORLD_LEAF_RUSTLE: SoundData = include_wav!("gfx/sound/world_leaf_rustle.wav");
    /// Ambient woodland bird / owl call.
    pub static WORLD_BIRDS_OWLS: SoundData = include_wav!("gfx/sound/world_birds_owls.wav");
}
