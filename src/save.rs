//! Cartridge SRAM persistence and game statistics for **CapiHunt**.
//!
//! Uses [`agb::save::SaveManager::init_sram`] with **3 save slots** so progress
//! works on real GBA hardware flashcarts (EverDrive, EZ-Flash) and emulators
//! such as mGBA. If a host environment lacks save media, [`SaveSystem`] falls
//! back transparently to an in-memory 3-slot store so gameplay never panics.

use agb::save::{SaveSlotManager, Slot};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Number of user-selectable save slots displayed on the Save Select screen.
pub const NUM_SAVE_SLOTS: usize = 3;

/// 32-byte ROM save magic identifier used to detect unformatted or incompatible SRAM.
const SAVE_MAGIC: [u8; 32] = *b"CAPIHUNT_GBA_V1_0000000000000000";

/// User-configurable audio toggles accessible from Title and Pause -> Options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioOptions {
    /// Whether XM background music playback is audible.
    pub music_on: bool,
    /// Whether WAV sound effects (player, Capis, ambient world) are audible.
    pub sound_on: bool,
}

impl Default for AudioOptions {
    fn default() -> Self {
        Self {
            music_on: true,
            sound_on: true,
        }
    }
}

/// Cumulative gameplay statistics tracked during a run and displayed on Game Over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStats {
    /// Total gameplay frames survived (60 frames ~= 1 second on GBA).
    pub frames_survived: u32,
    /// Total number of Capis successfully hunted (reduced to 0 health).
    pub capis_hunted: u16,
    /// Total number of raw meats finished cooking on campfires.
    pub meat_cooked: u16,
    /// Total number of cooked meats eaten by the player.
    pub meat_eaten: u16,
    /// Total number of arrows loosed from the hunting bow.
    pub arrows_fired: u16,
    /// Total number of unlit campfires ignited by the player.
    pub fires_lit: u16,
    /// Total times the player took fire damage.
    pub damage_taken: u16,
}

impl GameStats {
    /// Converts `frames_survived` into whole seconds at ~60 FPS.
    #[must_use]
    pub const fn seconds_survived(&self) -> u32 {
        self.frames_survived / 60
    }
}

/// Lightweight summary stored in each save slot's header for instant Save Select previews.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotSummary {
    /// Player's current hunger level (`1..=3`) when saved.
    pub hunger: u8,
    /// Capis hunted so far in this save file.
    pub capis_hunted: u16,
    /// Cooked meats eaten so far in this save file.
    pub meat_eaten: u16,
    /// Whole seconds survived in this save file.
    pub seconds_survived: u32,
}

/// Serialized state of a campfire modified by the player (lit or holding meat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedCampfire {
    /// World X coordinate in 8x8 tiles (`0..1024`).
    pub tile_x: u16,
    /// World Y coordinate in 8x8 tiles (`0..1024`).
    pub tile_y: u16,
    /// `true` if the campfire has been lit.
    pub lit: bool,
    /// Meat state on this fire: `0 = Empty`, `1 = Cooking`, `2 = CookedReady`.
    pub meat_state: u8,
    /// Remaining cooking frames (`0..=600`) when `meat_state == 1`.
    pub cook_frames_left: u16,
}

/// Complete game session snapshot written to and read from one of the 3 SRAM slots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedGameData {
    /// Deterministic world seed used to generate biomes, obstacles, and pickups.
    pub world_seed: u32,
    /// Player top-left world X coordinate in pixels.
    pub player_x: i32,
    /// Player top-left world Y coordinate in pixels.
    pub player_y: i32,
    /// Current hunger (`1..=3`).
    pub hunger: u8,
    /// Frame countdown until the next point of hunger drains.
    pub hunger_timer: u16,
    /// Whether the player is carrying raw meat (capacity: max 1).
    pub has_raw_meat: bool,
    /// Whether the player is carrying cooked meat (capacity: max 1).
    pub has_cooked_meat: bool,
    /// Current arrow count in the player's quiver.
    pub arrows: u8,
    /// `true` if the Hunting Blade is selected, `false` if the Hunting Bow is selected.
    pub blade_selected: bool,
    /// Accumulated run statistics.
    pub stats: GameStats,
    /// Audio preferences saved with this slot.
    pub options: AudioOptions,
    /// Modified campfires (kept bounded so SRAM writes remain fast and compact).
    pub campfires: Vec<SavedCampfire>,
}

/// Read-only status of a slot for UI rendering on the Save Select screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotInfo {
    /// Slot has never been saved to (or was erased).
    Empty,
    /// Slot holds a valid save with the given metadata summary.
    Valid(SlotSummary),
    /// Slot header failed integrity verification.
    Corrupted,
}

/// High-level 3-slot save wrapper around `agb`'s SRAM [`SaveSlotManager`].
pub struct SaveSystem {
    /// Hardware battery-backed SRAM manager when available.
    sram: Option<SaveSlotManager<SlotSummary>>,
    /// Fallback RAM slots used only if hardware SRAM initialisation fails.
    fallback_slots: [Option<(SlotSummary, SavedGameData)>; NUM_SAVE_SLOTS],
}

impl SaveSystem {
    /// Initialises 32 KiB battery-backed SRAM with 3 save slots.
    pub fn new(save_manager: &mut agb::save::SaveManager) -> Self {
        let sram = save_manager.init_sram::<SlotSummary>(NUM_SAVE_SLOTS, SAVE_MAGIC).ok();
        Self {
            sram,
            fallback_slots: [None, None, None],
        }
    }

    /// Queries the status and summary metadata of `slot` (`0..3`).
    #[must_use]
    pub fn slot_info(&self, slot: usize) -> SlotInfo {
        if slot >= NUM_SAVE_SLOTS {
            return SlotInfo::Empty;
        }
        if let Some(ref mgr) = self.sram {
            match mgr.slot(slot) {
                Slot::Empty => SlotInfo::Empty,
                Slot::Valid(meta) => SlotInfo::Valid(meta.clone()),
                Slot::Corrupted => SlotInfo::Corrupted,
            }
        } else {
            match &self.fallback_slots[slot] {
                Some((meta, _)) => SlotInfo::Valid(meta.clone()),
                None => SlotInfo::Empty,
            }
        }
    }

    /// Returns `true` if at least one of the 3 save slots contains valid data.
    #[must_use]
    pub fn has_any_valid_save(&self) -> bool {
        (0..NUM_SAVE_SLOTS).any(|idx| matches!(self.slot_info(idx), SlotInfo::Valid(_)))
    }

    /// Writes `data` and its derived [`SlotSummary`] into `slot` (`0..3`).
    pub fn save_slot(&mut self, slot: usize, data: &SavedGameData) -> bool {
        if slot >= NUM_SAVE_SLOTS {
            return false;
        }
        let summary = SlotSummary {
            hunger: data.hunger,
            capis_hunted: data.stats.capis_hunted,
            meat_eaten: data.stats.meat_eaten,
            seconds_survived: data.stats.seconds_survived(),
        };

        if let Some(ref mut mgr) = self.sram
            && mgr.write(slot, data, &summary).is_ok()
        {
            return true;
        }

        // Fallback to RAM slot so session saving still works even without cart SRAM.
        self.fallback_slots[slot] = Some((summary, data.clone()));
        true
    }

    /// Reads the full [`SavedGameData`] from `slot` (`0..3`), if valid.
    pub fn load_slot(&mut self, slot: usize) -> Option<SavedGameData> {
        if slot >= NUM_SAVE_SLOTS {
            return None;
        }
        if let Some(ref mut mgr) = self.sram
            && let Ok(data) = mgr.read::<SavedGameData>(slot)
        {
            return Some(data);
        }
        self.fallback_slots[slot].as_ref().map(|(_, d)| d.clone())
    }
}
