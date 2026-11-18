//! Audio manager combining `agb_tracker` XM music playback and `agb` WAV SFX.
//!
//! Runs the GBA software mixer at [`Frequency::Hz18157`], providing clear audio
//! while keeping ARM7TDMI CPU overhead low enough for smooth 60 FPS gameplay on
//! real Game Boy Advance hardware.

use agb::{
    fixnum::Num,
    sound::mixer::{Frequency, Mixer, SoundChannel, SoundData},
};
use agb_tracker::{Track, Tracker};

use crate::{
    assets::{music, sfx},
    save::AudioOptions,
    world::{Biome, hash_coords},
};

/// Mixer sample rate shared by all `.wav` sound effects and `agb_tracker`.
pub const MIXER_FREQUENCY: Frequency = Frequency::Hz18157;

/// High-level identifier for each `.xm` background music module in `gfx/music/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicId {
    /// Title & Save Select theme (`gfx/music/title.xm`).
    Title,
    /// Active wilderness hunting loop (`gfx/music/playing.xm`).
    Playing,
    /// Calm pause menu campfire loop (`gfx/music/pause.xm`).
    Pause,
    /// Sombre Game Over / Credits theme (`gfx/music/game_over.xm`).
    GameOver,
}

impl MusicId {
    /// Resolves the static [`Track`] asset for this music identifier.
    #[must_use]
    const fn track_ref(self) -> &'static Track {
        match self {
            MusicId::Title => &music::TITLE,
            MusicId::Playing => &music::PLAYING,
            MusicId::Pause => &music::PAUSE,
            MusicId::GameOver => &music::GAME_OVER,
        }
    }
}

/// High-level identifier for all 13 WAV sound effects in `gfx/sound/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SfxId {
    /// Player walking footstep on grass.
    PlayerWalk = 0,
    /// Player running (`Hold L`) footstep.
    PlayerRun = 1,
    /// Player hurt / burned by a lit campfire.
    PlayerHurt = 2,
    /// Player eating cooked meat (`Button R`).
    PlayerEat = 3,
    /// Player lighting an unlit campfire (`Button R`).
    PlayerStartFire = 4,
    /// Player shooting an arrow with the bow (`Hold A`).
    PlayerShootBow = 5,
    /// Player swinging the hunting blade (`Button A`).
    PlayerSwingBlade = 6,
    /// Nearby Capi gentle walking step.
    CapiWalk = 7,
    /// Startled Capi scurrying run step.
    CapiRun = 8,
    /// Capi squeak on taking damage.
    CapiHurt = 9,
    /// Ambient meadow wind swell.
    WorldWind = 10,
    /// Ambient forest leaf rustle.
    WorldLeafRustle = 11,
    /// Ambient woodland birds / owls call.
    WorldBirdsOwls = 12,
}

impl SfxId {
    /// Total number of distinct sound effects for cooldown indexing.
    const COUNT: usize = 13;

    /// Resolves the static [`SoundData`] buffer and default minimum frame gap.
    #[must_use]
    const fn data_and_cooldown(self) -> (SoundData, u8) {
        match self {
            SfxId::PlayerWalk => (sfx::PLAYER_WALK, 16),
            SfxId::PlayerRun => (sfx::PLAYER_RUN, 10),
            SfxId::PlayerHurt => (sfx::PLAYER_HURT, 12),
            SfxId::PlayerEat => (sfx::PLAYER_EAT, 12),
            SfxId::PlayerStartFire => (sfx::PLAYER_START_FIRE, 12),
            SfxId::PlayerShootBow => (sfx::PLAYER_SHOOT_BOW, 8),
            SfxId::PlayerSwingBlade => (sfx::PLAYER_SWING_BLADE, 8),
            SfxId::CapiWalk => (sfx::CAPI_WALK, 22),
            SfxId::CapiRun => (sfx::CAPI_RUN, 12),
            SfxId::CapiHurt => (sfx::CAPI_HURT, 6),
            SfxId::WorldWind => (sfx::WORLD_WIND, 60),
            SfxId::WorldLeafRustle => (sfx::WORLD_LEAF_RUSTLE, 45),
            SfxId::WorldBirdsOwls => (sfx::WORLD_BIRDS_OWLS, 60),
        }
    }
}

/// Central audio controller owning the GBA software [`Mixer`] and active [`Tracker`].
pub struct AudioManager<'gba> {
    /// Hardware-backed software mixer running at 18,157 Hz.
    mixer: Mixer<'gba>,
    /// Active XM tracker state, if a music module is loaded.
    tracker: Option<Tracker>,
    /// Currently selected music track so redundant switches do not restart the song.
    current_music: Option<MusicId>,
    /// Live user options controlling whether music and SFX are enabled.
    options: AudioOptions,
    /// Per-SFX frame cooldowns preventing channel starvation from overlapping footsteps.
    cooldowns: [u8; SfxId::COUNT],
    /// Countdown timer scheduling periodic ambient nature sounds during gameplay.
    ambient_timer: u16,
    /// Monotonic counter used to vary ambient sound selection deterministically.
    ambient_step: u32,
}

impl<'gba> AudioManager<'gba> {
    /// Creates a new [`AudioManager`] from the GBA [`MixerController`](agb::sound::mixer::MixerController).
    pub fn new(mixer_ctrl: &'gba mut agb::sound::mixer::MixerController, options: AudioOptions) -> Self {
        let mixer = mixer_ctrl.mixer(MIXER_FREQUENCY);
        Self {
            mixer,
            tracker: None,
            current_music: None,
            options,
            cooldowns: [0; SfxId::COUNT],
            ambient_timer: 150,
            ambient_step: 1,
        }
    }

    /// Returns the current [`AudioOptions`] (`music_on`, `sound_on`).
    #[must_use]
    pub const fn options(&self) -> AudioOptions {
        self.options
    }

    /// Applies updated [`AudioOptions`], immediately muting or restoring active music.
    pub fn set_options(&mut self, options: AudioOptions) {
        self.options = options;
        if let Some(ref mut tracker) = self.tracker {
            let vol: Num<i32, 8> = if options.music_on {
                Num::new(1)
            } else {
                Num::new(0)
            };
            tracker.set_volume(vol);
            if !options.music_on {
                tracker.stop(&mut self.mixer);
            }
        }
    }

    /// Toggles background music ON/OFF and returns the updated setting.
    pub fn toggle_music(&mut self) -> bool {
        let mut opts = self.options;
        opts.music_on = !opts.music_on;
        let current = self.current_music;
        self.set_options(opts);
        // When turning music back ON, cleanly restart the current screen's track.
        if opts.music_on
            && let Some(track_id) = current
        {
            self.current_music = None;
            self.play_music(track_id);
        }
        opts.music_on
    }

    /// Toggles WAV sound effects ON/OFF and returns the updated setting.
    pub fn toggle_sound(&mut self) -> bool {
        self.options.sound_on = !self.options.sound_on;
        self.options.sound_on
    }

    /// Switches to `music_id` if it is not already the active track.
    pub fn play_music(&mut self, music_id: MusicId) {
        if self.current_music == Some(music_id) {
            return;
        }
        if let Some(mut prev) = self.tracker.take() {
            prev.stop(&mut self.mixer);
        }
        self.current_music = Some(music_id);
        let mut tracker = Tracker::new(music_id.track_ref(), MIXER_FREQUENCY);
        if music_id == MusicId::GameOver {
            tracker.set_should_loop(false);
        }
        let vol: Num<i32, 8> = if self.options.music_on {
            Num::new(1)
        } else {
            Num::new(0)
        };
        tracker.set_volume(vol);
        self.tracker = Some(tracker);
    }

    /// Plays a centred sound effect at `volume_num / 16` if sound is enabled and off cooldown.
    pub fn play_sfx_scaled(&mut self, id: SfxId, volume_sixteenths: i16, pan_eighths: i16) {
        if !self.options.sound_on {
            return;
        }
        let slot = id as usize;
        if self.cooldowns[slot] > 0 {
            return;
        }
        let (sound_data, cd) = id.data_and_cooldown();
        self.cooldowns[slot] = cd;

        let mut channel = SoundChannel::new(sound_data);
        let vol = Num::<i16, 8>::new(volume_sixteenths.clamp(1, 16)) / 16;
        let pan = Num::<i16, 8>::new(pan_eighths.clamp(-8, 8)) / 8;
        channel.volume(vol).panning(pan);
        let _ = self.mixer.play_sound(channel);
    }

    /// Plays a standard centred sound effect at comfortable gameplay volume.
    pub fn play_sfx(&mut self, id: SfxId) {
        self.play_sfx_scaled(id, 13, 0);
    }

    /// Schedules occasional ambient wilderness sounds (wind, leaf rustle, birds/owls)
    /// matched to the biome surrounding the player.
    pub fn tick_world_ambience(&mut self, biome: Biome) {
        if self.ambient_timer > 0 {
            self.ambient_timer -= 1;
            return;
        }
        self.ambient_step = self.ambient_step.wrapping_add(1);
        let h = hash_coords(0x517C_C1B7, self.ambient_step as i32, biome as i32);
        // Next ambient cue in ~4 to ~8 seconds (240..495 frames).
        self.ambient_timer = 240 + ((h & 0xFF) as u16);

        let pan = (((h >> 8) & 0x07) as i16) - 3;
        let chosen = match (biome, (h >> 12) % 3) {
            (Biome::Forest, 0) => SfxId::WorldLeafRustle,
            (Biome::Forest, 1) => SfxId::WorldBirdsOwls,
            (Biome::Forest, _) => SfxId::WorldWind,
            (Biome::Meadow, 0) => SfxId::WorldWind,
            (Biome::Meadow, 1) => SfxId::WorldBirdsOwls,
            (Biome::Meadow, _) => SfxId::WorldLeafRustle,
        };
        self.play_sfx_scaled(chosen, 8, pan);
    }

    /// Advances cooldowns, steps the XM tracker, and mixes one frame of audio.
    ///
    /// Must be called once per frame just prior to `frame.commit()` / VBlank so audio
    /// DMA buffers never underrun on real hardware.
    pub fn frame(&mut self) {
        for c in &mut self.cooldowns {
            *c = c.saturating_sub(1);
        }
        if self.options.music_on
            && let Some(ref mut tracker) = self.tracker
        {
            tracker.step(&mut self.mixer);
        }
        self.mixer.frame();
    }
}
