//! Main Playing & Pause game state (`src/playing.rs`) for **CapiHunt**.
//!
//! Coordinates the `1024x1024` streaming [`WorldMap`], [`Player`], peaceful [`Capi`]
//! herds, [`Campfire`] cooking & burn hazards, [`ArrowProjectile`]s, [`WorldPickup`]s,
//! ambient biome audio, the `16px` top HUD bar, and the full **Pause** overlay
//! (`Resume`, `Save` to any of the 3 SRAM slots, `Options` [Music/Sound], `Exit`).

use agb::{
    display::{
        Graphics, Priority,
        tiled::{RegularBackground, RegularBackgroundSize, TileFormat},
    },
    fixnum::{Rect, Vector2D, rect, vec2},
    input::{Button, ButtonController},
};
use alloc::{format, vec::Vec};

use crate::{
    AppState,
    assets::backgrounds,
    audio::{AudioManager, MusicId, SfxId},
    entities::{
        ArrowProjectile, Campfire, Capi, ContextPrompt, MAX_ARROWS, PickupKind, Player, Weapon,
        WorldPickup, show_hud_sprites, sync_nearby_arrow_pickups, sync_nearby_campfires,
    },
    save::{GameStats, NUM_SAVE_SLOTS, SaveSystem, SavedCampfire, SavedGameData},
    ui::{PAL_GOLD, PAL_MUTED, PAL_WHITE, TextCanvas, init_palettes, show_menu_cursor},
    world::{
        START_TILE_X, START_TILE_Y, TILE_PX, WORLD_PX_H, WORLD_PX_W, WorldMap, biome_at,
        hash_coords, rect_collides_world,
    },
};

/// Maximum peaceful Capis simulated simultaneously around the player.
const MAX_ACTIVE_CAPIS: usize = 7;

/// Sub-view inside the Pause overlay (`Button Start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseMenuState {
    /// Gameplay is running normally (not paused).
    Unpaused,
    /// Primary pause menu (`Resume`, `Save`, `Options`, `Exit`).
    Main {
        selected: usize,
        save_slot: usize,
        saved_banner: Option<usize>,
    },
    /// Pause -> Options submenu (`Music`, `Sound`, `Back`).
    Options { selected: usize, save_slot: usize },
}

/// Computes the clamped `240x160` camera top-left position centered on `player`.
#[must_use]
fn camera_for_player(player: &Player) -> Vector2D<i32> {
    let p = player.pixel_pos();
    let cx = (p.x + 4 - 120).clamp(0, WORLD_PX_W - 240);
    let cy = (p.y + 8 - 88).clamp(0, WORLD_PX_H - 160);
    vec2(cx, cy)
}

/// Redraws the `16px` top HUD text strip (`arrow count` + contextual `[R]` prompt).
fn refresh_hud_bar(canvas: &mut TextCanvas, arrows: u8, prompt: ContextPrompt, capis_hunted: u16) {
    canvas.clear();
    canvas.fill_hud_bar(0, 2);

    let arrow_txt = format!("x{:02}", arrows);
    canvas.draw_left(52, 4, 36, PAL_WHITE, &arrow_txt);

    let hunt_txt = format!("H:{}", capis_hunted);
    canvas.draw_left(124, 4, 48, PAL_MUTED, &hunt_txt);

    let p_lbl = prompt.label();
    if !p_lbl.is_empty() {
        canvas.draw_left(174, 4, 64, PAL_GOLD, p_lbl);
    }
}

/// Redraws the Pause overlay text onto `pause_canvas`.
fn render_pause_text(
    canvas: &mut TextCanvas,
    state: PauseMenuState,
    music_on: bool,
    sound_on: bool,
) {
    canvas.clear();
    match state {
        PauseMenuState::Unpaused => {}
        PauseMenuState::Main {
            selected,
            save_slot,
            saved_banner,
        } => {
            canvas.draw_centred(24, 24, 192, PAL_GOLD, "PAUSED");
            let save_lbl = format!("SAVE <SLOT {}>", save_slot + 1);
            let rows: [&str; 4] = ["RESUME", &save_lbl, "OPTIONS", "EXIT TO TITLE"];
            for (idx, item) in rows.iter().enumerate() {
                let pal = if idx == selected { PAL_GOLD } else { PAL_WHITE };
                canvas.draw_left(64, 52 + (idx as i32) * 18, 144, pal, item);
            }
            if let Some(slot_num) = saved_banner {
                let msg = format!("SAVED TO SLOT {}!", slot_num + 1);
                canvas.draw_centred(24, 126, 192, PAL_MUTED, &msg);
            }
        }
        PauseMenuState::Options { selected, .. } => {
            canvas.draw_centred(24, 24, 192, PAL_GOLD, "PAUSE OPTIONS");
            let m_lbl = if music_on { "MUSIC: ON" } else { "MUSIC: OFF" };
            let s_lbl = if sound_on { "SOUND: ON" } else { "SOUND: OFF" };
            let rows = [m_lbl, s_lbl, "BACK"];
            for (idx, item) in rows.iter().enumerate() {
                let pal = if idx == selected { PAL_GOLD } else { PAL_WHITE };
                canvas.draw_left(72, 60 + (idx as i32) * 18, 136, pal, item);
            }
        }
    }
}

/// Attempts to spawn a peaceful Capi at a clear grass/moss spot near `player_pos`.
fn try_spawn_capi(
    world_seed: u32,
    player_pos: Vector2D<i32>,
    attempt_salt: u32,
    min_dist: i32,
    max_dist: i32,
    capis: &mut Vec<Capi>,
) {
    if capis.len() >= MAX_ACTIVE_CAPIS {
        return;
    }
    for i in 0..8u32 {
        let h = hash_coords(world_seed ^ attempt_salt.wrapping_mul(0x045D_9F3B), player_pos.x, (player_pos.y).wrapping_add(i as i32 * 37));
        let dist_x = min_dist + ((h & 0x3F) as i32).min(max_dist - min_dist);
        let dist_y = min_dist / 2 + (((h >> 6) & 0x3F) as i32).min(max_dist - min_dist / 2);
        let sx = if (h & 0x1000) == 0 { dist_x } else { -dist_x };
        let sy = if (h & 0x2000) == 0 { dist_y } else { -dist_y };
        let cand_x = ((player_pos.x + sx) & !7).clamp(40, WORLD_PX_W - 48);
        let cand_y = ((player_pos.y + sy) & !7).clamp(40, WORLD_PX_H - 48);

        let test_box = rect(vec2(cand_x, cand_y), vec2(8, 8));
        if !rect_collides_world(world_seed, test_box) {
            capis.push(Capi::new(cand_x, cand_y, h));
            return;
        }
    }
}

/// Starts a brand-new Playing session in `slot` with `world_seed`.
pub fn run_playing_new(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
    saves: &mut SaveSystem,
    slot: usize,
    world_seed: u32,
) -> AppState {
    let start_px = START_TILE_X * TILE_PX;
    let start_py = START_TILE_Y * TILE_PX;
    let player = Player::new(start_px, start_py);
    run_session(
        gfx,
        input,
        audio,
        saves,
        slot,
        world_seed,
        player,
        GameStats::default(),
        Vec::new(),
    )
}

/// Resumes a Playing session loaded from SRAM `slot`.
pub fn run_playing_loaded(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
    saves: &mut SaveSystem,
    slot: usize,
    data: SavedGameData,
) -> AppState {
    let mut player = Player::new(data.player_x, data.player_y);
    player.hunger = data.hunger.clamp(1, 3);
    player.hunger_timer = data.hunger_timer;
    player.has_raw_meat = data.has_raw_meat;
    player.has_cooked_meat = data.has_cooked_meat;
    player.arrows = data.arrows.min(MAX_ARROWS);
    player.weapon = if data.blade_selected {
        Weapon::Blade
    } else {
        Weapon::Bow
    };
    let campfires: Vec<Campfire> = data.campfires.into_iter().map(Campfire::from_saved).collect();

    run_session(
        gfx,
        input,
        audio,
        saves,
        slot,
        data.world_seed,
        player,
        data.stats,
        campfires,
    )
}

/// Core 60 FPS gameplay + pause loop shared by new and loaded sessions.
#[allow(clippy::too_many_arguments)]
fn run_session(
    gfx: &mut Graphics,
    input: &mut ButtonController,
    audio: &mut AudioManager,
    saves: &mut SaveSystem,
    default_slot: usize,
    world_seed: u32,
    mut player: Player,
    mut stats: GameStats,
    mut campfires: Vec<Campfire>,
) -> AppState {
    init_palettes(gfx);
    audio.play_music(MusicId::Playing);

    let mut cam = camera_for_player(&player);
    let mut world = WorldMap::new(world_seed, cam);

    let mut arrows: Vec<ArrowProjectile> = Vec::new();
    let mut pickups: Vec<WorldPickup> = Vec::new();
    let mut collected_arrow_chunks: Vec<(i16, i16)> = Vec::new();
    let mut capis: Vec<Capi> = Vec::new();

    // Populate nearby campfires, arrow pickups, and initial meadow Capis right away.
    sync_nearby_campfires(world_seed, player.pixel_pos(), &mut campfires);
    sync_nearby_arrow_pickups(
        world_seed,
        player.pixel_pos(),
        &mut pickups,
        &collected_arrow_chunks,
    );
    for s in 1..=4 {
        try_spawn_capi(world_seed, player.pixel_pos(), s * 97, 56, 112, &mut capis);
    }

    let mut hud_text = TextCanvas::new();
    let mut cached_hud_state = (u8::MAX, ContextPrompt::None, u16::MAX);

    // Optional pause background + pause text allocated only while paused to conserve BG VRAM.
    let mut pause_bg: Option<RegularBackground> = None;
    let mut pause_text: Option<TextCanvas> = None;
    let mut pause_state = PauseMenuState::Unpaused;

    let mut global_tick: u32 = 0;

    loop {
        input.update();
        global_tick = global_tick.wrapping_add(1);

        // -------------------------------------------------------------------
        // Pause Menu Handling (`Button Start`)
        // -------------------------------------------------------------------
        if !matches!(pause_state, PauseMenuState::Unpaused) {
            let mut redraw_pause = false;
            let mut resume_game = false;

            match pause_state {
                PauseMenuState::Unpaused => unreachable!(),
                PauseMenuState::Main {
                    mut selected,
                    mut save_slot,
                    mut saved_banner,
                } => {
                    if input.is_just_pressed(Button::Start) || input.is_just_pressed(Button::B) {
                        resume_game = true;
                    } else if input.is_just_pressed(Button::Up) {
                        selected = if selected == 0 { 3 } else { selected - 1 };
                        saved_banner = None;
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if input.is_just_pressed(Button::Down) {
                        selected = (selected + 1) % 4;
                        saved_banner = None;
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if selected == 1 && input.is_just_pressed(Button::Left) {
                        save_slot = if save_slot == 0 {
                            NUM_SAVE_SLOTS - 1
                        } else {
                            save_slot - 1
                        };
                        saved_banner = None;
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if selected == 1 && input.is_just_pressed(Button::Right) {
                        save_slot = (save_slot + 1) % NUM_SAVE_SLOTS;
                        saved_banner = None;
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if input.is_just_pressed(Button::A) {
                        match selected {
                            0 => resume_game = true,
                            1 => {
                                // Persist current session into the chosen SRAM slot!
                                let saved_fires: Vec<SavedCampfire> = campfires
                                    .iter()
                                    .filter_map(Campfire::to_saved)
                                    .take(32)
                                    .collect();
                                let p = player.pixel_pos();
                                let snapshot = SavedGameData {
                                    world_seed,
                                    player_x: p.x,
                                    player_y: p.y,
                                    hunger: player.hunger,
                                    hunger_timer: player.hunger_timer,
                                    has_raw_meat: player.has_raw_meat,
                                    has_cooked_meat: player.has_cooked_meat,
                                    arrows: player.arrows,
                                    blade_selected: player.weapon == Weapon::Blade,
                                    stats,
                                    options: audio.options(),
                                    campfires: saved_fires,
                                };
                                let _ = saves.save_slot(save_slot, &snapshot);
                                saved_banner = Some(save_slot);
                                audio.play_sfx(SfxId::PlayerEat);
                                redraw_pause = true;
                            }
                            2 => {
                                pause_state = PauseMenuState::Options {
                                    selected: 0,
                                    save_slot,
                                };
                                audio.play_sfx(SfxId::PlayerSwingBlade);
                                redraw_pause = true;
                            }
                            _ => {
                                audio.play_sfx(SfxId::PlayerWalk);
                                return AppState::Title;
                            }
                        }
                    }

                    if !resume_game
                        && let PauseMenuState::Main { .. } = pause_state
                    {
                        pause_state = PauseMenuState::Main {
                            selected,
                            save_slot,
                            saved_banner,
                        };
                    }
                }
                PauseMenuState::Options {
                    mut selected,
                    save_slot,
                } => {
                    if input.is_just_pressed(Button::B) {
                        pause_state = PauseMenuState::Main {
                            selected: 2,
                            save_slot,
                            saved_banner: None,
                        };
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if input.is_just_pressed(Button::Up) {
                        selected = if selected == 0 { 2 } else { selected - 1 };
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if input.is_just_pressed(Button::Down) {
                        selected = (selected + 1) % 3;
                        audio.play_sfx(SfxId::PlayerWalk);
                        redraw_pause = true;
                    } else if input.is_just_pressed(Button::A)
                        || input.is_just_pressed(Button::Left)
                        || input.is_just_pressed(Button::Right)
                    {
                        match selected {
                            0 => {
                                audio.toggle_music();
                                audio.play_sfx(SfxId::PlayerWalk);
                                redraw_pause = true;
                            }
                            1 => {
                                audio.toggle_sound();
                                audio.play_sfx(SfxId::PlayerWalk);
                                redraw_pause = true;
                            }
                            _ => {
                                if input.is_just_pressed(Button::A) {
                                    pause_state = PauseMenuState::Main {
                                        selected: 2,
                                        save_slot,
                                        saved_banner: None,
                                    };
                                    audio.play_sfx(SfxId::PlayerWalk);
                                    redraw_pause = true;
                                }
                            }
                        }
                    }
                    if let PauseMenuState::Options { .. } = pause_state {
                        pause_state = PauseMenuState::Options {
                            selected,
                            save_slot,
                        };
                    }
                }
            }

            if resume_game {
                pause_state = PauseMenuState::Unpaused;
                pause_bg = None;
                pause_text = None;
                audio.play_sfx(SfxId::PlayerWalk);
                audio.play_music(MusicId::Playing);
                cached_hud_state.0 = u8::MAX; // Force clean HUD refresh
            } else {
                if redraw_pause && let Some(ref mut pt) = pause_text {
                    render_pause_text(
                        pt,
                        pause_state,
                        audio.options().music_on,
                        audio.options().sound_on,
                    );
                }

                audio.frame();
                let mut frame = gfx.frame();
                if let Some(ref pbg) = pause_bg {
                    pbg.show(&mut frame);
                }
                if let Some(ref pt) = pause_text {
                    pt.show(&mut frame);
                }
                match pause_state {
                    PauseMenuState::Main { selected, .. } => {
                        show_menu_cursor(&mut frame, 52, 52 + (selected as i32) * 18, global_tick);
                    }
                    PauseMenuState::Options { selected, .. } => {
                        show_menu_cursor(&mut frame, 60, 60 + (selected as i32) * 18, global_tick);
                    }
                    PauseMenuState::Unpaused => {}
                }
                frame.commit();
                continue;
            }
        }

        // Enter Pause overlay when `Button Start` is pressed during gameplay.
        if input.is_just_pressed(Button::Start) {
            pause_state = PauseMenuState::Main {
                selected: 0,
                save_slot: default_slot.min(NUM_SAVE_SLOTS - 1),
                saved_banner: None,
            };
            audio.play_sfx(SfxId::PlayerWalk);
            audio.play_music(MusicId::Pause);

            let mut bg = RegularBackground::new(
                Priority::P1,
                RegularBackgroundSize::Background32x32,
                TileFormat::FourBpp,
            );
            bg.fill_with(&backgrounds::PAUSE_BG);
            let mut pt = TextCanvas::new();
            render_pause_text(
                &mut pt,
                pause_state,
                audio.options().music_on,
                audio.options().sound_on,
            );
            pause_bg = Some(bg);
            pause_text = Some(pt);

            audio.frame();
            let mut frame = gfx.frame();
            if let Some(ref pbg) = pause_bg {
                pbg.show(&mut frame);
            }
            if let Some(ref pt) = pause_text {
                pt.show(&mut frame);
            }
            show_menu_cursor(&mut frame, 52, 52, global_tick);
            frame.commit();
            continue;
        }

        // -------------------------------------------------------------------
        // Active Gameplay Simulation Step
        // -------------------------------------------------------------------
        stats.frames_survived = stats.frames_survived.saturating_add(1);

        // Periodically stream nearby chunk campfires, arrow bundles, and fresh Capis.
        if (global_tick & 0x1F) == 0 {
            let ppos = player.pixel_pos();
            sync_nearby_campfires(world_seed, ppos, &mut campfires);
            sync_nearby_arrow_pickups(world_seed, ppos, &mut pickups, &collected_arrow_chunks);
        }
        if (global_tick & 0x3F) == 0 {
            let ppos = player.pixel_pos();
            capis.retain(|c| {
                let cp = c.pixel_pos();
                (cp.x - ppos.x).abs() < 380 && (cp.y - ppos.y).abs() < 380
            });
            try_spawn_capi(world_seed, ppos, global_tick, 136, 220, &mut capis);
        }

        // Tick campfire 10-second meat cooking timers.
        for fire in &mut campfires {
            fire.tick(&mut stats);
        }

        // Update player movement, attacks, and `[R]` interactions.
        let prompt = player.update(
            input,
            world_seed,
            &mut campfires,
            &mut arrows,
            audio,
            &mut stats,
        );

        // Check lit campfire burn hazard against player's boots.
        let p_feet = player.feet_rect();
        for fire in &campfires {
            if fire.lit && p_feet.touches(fire.hazard_rect()) {
                player.apply_fire_damage(fire.centre(), audio, &mut stats);
                break;
            }
        }

        // Check starvation / Game Over condition (`hunger == 0`).
        if player.hunger == 0 {
            return AppState::GameOver(stats);
        }

        // Check player melee blade hits against Capis.
        let blade_hit = player.active_blade_hitbox();
        let p_centre = player.centre();
        let p_running = player.is_running();

        let mut i = 0;
        while i < capis.len() {
            let mut defeated = false;
            if let Some((blade_rect, swing_id)) = blade_hit
                && capis[i].hitbox().touches(blade_rect)
            {
                defeated = capis[i].take_hit(Some(swing_id), audio);
            }
            if defeated {
                let drop_pos = capis[i].pixel_pos();
                capis.swap_remove(i);
                stats.capis_hunted = stats.capis_hunted.saturating_add(1);
                pickups.push(WorldPickup {
                    pos: drop_pos,
                    kind: PickupKind::RawMeat,
                });
            } else {
                capis[i].update(world_seed, p_centre, p_running, cam, audio);
                i += 1;
            }
        }

        // Step flying arrows and check arrow hits against Capis.
        let mut ai = 0;
        while ai < arrows.len() {
            if !arrows[ai].update(world_seed) {
                arrows.swap_remove(ai);
                continue;
            }
            let a_box = arrows[ai].hitbox();
            let mut hit_any = false;
            let mut ci = 0;
            while ci < capis.len() {
                if capis[ci].hitbox().touches(a_box) {
                    hit_any = true;
                    if capis[ci].take_hit(None, audio) {
                        let drop_pos = capis[ci].pixel_pos();
                        capis.swap_remove(ci);
                        stats.capis_hunted = stats.capis_hunted.saturating_add(1);
                        pickups.push(WorldPickup {
                            pos: drop_pos,
                            kind: PickupKind::RawMeat,
                        });
                    }
                    break;
                }
                ci += 1;
            }
            if hit_any {
                arrows.swap_remove(ai);
            } else {
                ai += 1;
            }
        }

        // Collect world pickups (1 raw meat max; limited arrows up to MAX_ARROWS).
        let player_body: Rect<i32> = rect(player.pixel_pos(), vec2(8, 16));
        let mut pi = 0;
        while pi < pickups.len() {
            if pickups[pi].hitbox().touches(player_body) {
                match pickups[pi].kind {
                    PickupKind::RawMeat if !player.has_raw_meat => {
                        player.has_raw_meat = true;
                        audio.play_sfx(SfxId::PlayerWalk);
                        pickups.swap_remove(pi);
                        continue;
                    }
                    PickupKind::ArrowBundle { chunk_x, chunk_y } if player.arrows < MAX_ARROWS => {
                        player.arrows = (player.arrows + 3).min(MAX_ARROWS);
                        if collected_arrow_chunks.len() >= 24 {
                            collected_arrow_chunks.remove(0);
                        }
                        collected_arrow_chunks.push((chunk_x, chunk_y));
                        audio.play_sfx(SfxId::PlayerWalk);
                        pickups.swap_remove(pi);
                        continue;
                    }
                    _ => {}
                }
            }
            pi += 1;
        }

        // Ambient nature sound effects matched to current biome.
        let p_tile = player.pixel_pos() / TILE_PX;
        audio.tick_world_ambience(biome_at(world_seed, p_tile.x, p_tile.y));

        // Update camera and stream newly visible world tiles.
        cam = camera_for_player(&player);
        world.update_scroll(cam);

        // Refresh top HUD text strip only when values change.
        let hud_key = (player.arrows, prompt, stats.capis_hunted);
        if hud_key != cached_hud_state {
            cached_hud_state = hud_key;
            refresh_hud_bar(&mut hud_text, player.arrows, prompt, stats.capis_hunted);
        }

        // Mix audio just before VBlank commit!
        audio.frame();

        // -------------------------------------------------------------------
        // Render Frame (HUD on P0, Entities on P1, World Tiles on P2)
        // -------------------------------------------------------------------
        let mut frame = gfx.frame();

        show_hud_sprites(&mut frame, &player);
        player.show(&mut frame, cam);
        for arrow in &arrows {
            arrow.show(&mut frame, cam);
        }
        for capi in &capis {
            capi.show(&mut frame, cam);
        }
        for fire in &campfires {
            fire.show(&mut frame, cam, global_tick);
        }
        for pickup in &pickups {
            pickup.show(&mut frame, cam, global_tick);
        }

        hud_text.show(&mut frame);
        world.show(&mut frame);

        frame.commit();
    }
}
