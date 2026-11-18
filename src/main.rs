//! **CapiHunt** — A Game Boy Advance wilderness survival hunting game built with `agb`.
//!
//! Hunt peaceful Capybaras ("Capis") across a `1024x1024` tile world of meadows and
//! forests, light campfires, cook meat (~10 seconds), and eat before hunger runs out.

// Games made using `agb` are no_std which means you don't have access to the standard
// rust library. This is because the game boy advance doesn't have an operating
// system, so most of the content of the standard library doesn't apply.
#![no_std]
// `agb` defines its own `main` function, so you must declare your game's main function
// using the #[agb::entry] proc macro. Failing to do so will cause failure in linking
// which won't be a particularly clear error message.
#![no_main]
// This is required to allow writing tests
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

// By default no_std crates don't get alloc, so you won't be able to use things like Vec
// until you declare the extern crate. `agb` provides an allocator so it will all work
extern crate alloc;

pub mod assets;
pub mod audio;
pub mod credits;
pub mod entities;
pub mod game_over;
pub mod playing;
pub mod save;
pub mod save_select;
pub mod title;
pub mod ui;
pub mod world;

use agb::input::ButtonController;
use audio::AudioManager;
use save::{AudioOptions, GameStats, SaveSystem, SavedGameData};
use save_select::SaveSelectMode;

/// Top-level state machine governing transitions between all 5 game state modules.
pub enum AppState {
    /// Title menu (`New Start`, `Load Save`, `Options`, `Credits`).
    Title,
    /// 3-slot SRAM Save Select screen (`NewGame` or `LoadGame`).
    SaveSelect(SaveSelectMode),
    /// Start a fresh hunt bound to `slot` with deterministic `world_seed`.
    PlayingNew {
        /// Selected SRAM save slot index (`0..3`).
        slot: usize,
        /// World generation seed.
        world_seed: u32,
    },
    /// Resume an existing hunt loaded from `slot`.
    PlayingLoaded {
        /// Selected SRAM save slot index (`0..3`).
        slot: usize,
        /// Full deserialized game state.
        data: SavedGameData,
    },
    /// Game Over summary displaying `stats` with `Credits` and `Exit` options.
    GameOver(GameStats),
    /// Scrolling Credits screen exiting back to `Title`.
    Credits,
}

/// GBA hardware entry point invoked by the `agb` runtime after boot.
#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    let mut gfx = gba.graphics.get();
    let mut saves = SaveSystem::new(&mut gba.save);
    let mut audio = AudioManager::new(&mut gba.mixer, AudioOptions::default());
    let mut input = ButtonController::new();

    let mut state = AppState::Title;

    loop {
        state = match state {
            AppState::Title => title::run_title(&mut gfx, &mut input, &mut audio, &saves),
            AppState::SaveSelect(mode) => {
                save_select::run_save_select(&mut gfx, &mut input, &mut audio, &mut saves, mode)
            }
            AppState::PlayingNew { slot, world_seed } => playing::run_playing_new(
                &mut gfx,
                &mut input,
                &mut audio,
                &mut saves,
                slot,
                world_seed,
            ),
            AppState::PlayingLoaded { slot, data } => playing::run_playing_loaded(
                &mut gfx,
                &mut input,
                &mut audio,
                &mut saves,
                slot,
                data,
            ),
            AppState::GameOver(stats) => {
                game_over::run_game_over(&mut gfx, &mut input, &mut audio, stats)
            }
            AppState::Credits => credits::run_credits(&mut gfx, &mut input, &mut audio),
        };
    }
}
