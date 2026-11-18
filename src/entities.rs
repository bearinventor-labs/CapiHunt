//! Gameplay entities for **CapiHunt**: Player, Capybaras ("Capis"), Campfires,
//! Projectiles, World Pickups, and the in-game Sprite HUD.
//!
//! All coordinates use subpixel fixed-point (`Num<i32, 8>`) for smooth movement
//! on the GBA's 240x160 LCD while collision checks operate on integer pixel
//! [`Rect`]s against the base collision boxes defined in [`crate::world`].

use agb::{
    display::{
        GraphicsFrame, Priority,
        object::{Object, Sprite, Tag},
    },
    fixnum::{Num, Rect, Vector2D, rect, vec2},
    input::{Button, ButtonController, Tri},
};
use alloc::vec::Vec;

use crate::{
    assets::{hud_icon, sprites},
    audio::{AudioManager, SfxId},
    save::{GameStats, SavedCampfire},
    world::{
        TILE_PX, WORLD_PX_H, WORLD_PX_W, chunk_arrow_pickup_tile, chunk_campfire_tile, hash_coords,
        rect_collides_world,
    },
};

/// Maximum player hunger pips (`docs/INTENT.md`: `hunger = 3`).
pub const MAX_HUNGER: u8 = 3;
/// Frames before 1 point of hunger drains (~45 seconds at 60 FPS).
pub const HUNGER_DRAIN_FRAMES: u16 = 60 * 45;
/// Starting arrow count for a fresh hunt.
pub const START_ARROWS: u8 = 5;
/// Maximum arrows the hunter's quiver can hold at once.
pub const MAX_ARROWS: u8 = 12;
/// Number of hits required to hunt one Capi (`docs/INTENT.md`: `health = 4 hits`).
pub const CAPI_MAX_HEALTH: u8 = 4;
/// Frames required to cook 1 raw meat on a lit campfire (`~10 seconds` at 60 FPS).
pub const FIRE_COOK_FRAMES: u16 = 60 * 10;

/// Cardinal facing direction of the player.
///
/// Sprite sheets exist for `Front`, `Right`, and `Back`; `Left` reuses `Right`
/// with hardware horizontal flipping (`Object::set_hflip(true)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    /// Facing down towards the camera (`front`).
    Front,
    /// Facing right (`right`).
    Right,
    /// Facing left (`right` + `hflip`).
    Left,
    /// Facing up away from the camera (`back`).
    Back,
}

impl Facing {
    /// Unit vector `(dx, dy)` corresponding to this facing direction.
    #[must_use]
    pub const fn unit_vec(self) -> Vector2D<i32> {
        match self {
            Facing::Front => vec2(0, 1),
            Facing::Right => vec2(1, 0),
            Facing::Left => vec2(-1, 0),
            Facing::Back => vec2(0, -1),
        }
    }
}

/// Active weapon selected via `Button Select`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weapon {
    /// Close-range hunting blade (`Button A` tap).
    Blade,
    /// Ranged hunting bow (`Hold A` draw and release; consumes 1 arrow).
    Bow,
}

/// All 8 player animation states mandated by `docs/INTENT.md` (4 frames each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAnim {
    /// Standing idle (`idle_*`).
    Idle,
    /// Walking (`walk_*`).
    Walk,
    /// Running while holding `L` (`run_*`).
    Run,
    /// Drawing / loosing the hunting bow (`shoot_bow_*`).
    ShootBow,
    /// Swinging the hunting blade (`swing_blade_*`).
    SwingBlade,
    /// Recoiling from fire damage (`take_damage_*`).
    TakeDamage,
    /// Eating cooked meat (`eat_*`).
    Eat,
    /// Striking flint to light an unlit campfire (`start_fire_*`).
    StartFire,
}

/// Timed non-movement action currently locking or overriding the player's animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveAction {
    /// No special action; player can move/attack freely.
    None,
    /// Swinging blade (`frames_left` out of 16, hitbox active on frames `4..=12`).
    BladeSwing { frames_left: u8, hit_id: u16 },
    /// Drawing bow while `Button A` is held (`draw_frames` counting up).
    BowDraw { draw_frames: u8 },
    /// Brief bow release follow-through (`frames_left` out of 8).
    BowRelease { frames_left: u8 },
    /// Eating cooked meat (`frames_left` out of 24).
    Eating { frames_left: u8 },
    /// Lighting an unlit campfire (`frames_left` out of 24).
    LightingFire { frames_left: u8 },
    /// Taking burn damage recoil (`frames_left` out of 20).
    Hurt { frames_left: u8, knock_dx: i32, knock_dy: i32 },
}

/// Contextual `[R]` prompt describing what pressing `Button R` will do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextPrompt {
    /// Nothing actionable on `R` right now.
    None,
    /// Nearby unlit campfire can be lit with `R`.
    LightFire,
    /// Nearby lit campfire is empty and player carries raw meat -> cook with `R`.
    CookMeat,
    /// Nearby lit campfire is currently cooking meat.
    CookingInProgress,
    /// Nearby lit campfire has finished cooking meat -> grab with `R`.
    GrabCooked,
    /// Player carries cooked meat -> eat with `R`.
    EatMeat,
}

impl ContextPrompt {
    /// Short HUD status text shown along the top bar.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ContextPrompt::None => "",
            ContextPrompt::LightFire => "R:LIGHT",
            ContextPrompt::CookMeat => "R:COOK",
            ContextPrompt::CookingInProgress => "COOKING",
            ContextPrompt::GrabCooked => "R:GRAB",
            ContextPrompt::EatMeat => "R:EAT",
        }
    }
}

/// The `8x16` hunter controlled by the player.
pub struct Player {
    /// Subpixel top-left world position of the `8x16` sprite.
    pub pos: Vector2D<Num<i32, 8>>,
    /// Current cardinal facing direction.
    pub facing: Facing,
    /// Currently selected weapon (`Select` toggles Blade <-> Bow).
    pub weapon: Weapon,
    /// Current hunger (`0..=3`; reaching `0` triggers Game Over).
    pub hunger: u8,
    /// Countdown frames until hunger decreases by 1.
    pub hunger_timer: u16,
    /// Carries at most 1 raw meat at a time.
    pub has_raw_meat: bool,
    /// Carries at most 1 cooked meat at a time.
    pub has_cooked_meat: bool,
    /// Limited arrows currently in quiver.
    pub arrows: u8,
    /// Invulnerability frames after taking fire damage.
    pub invuln_frames: u8,
    /// Monotonic counter used to tag each melee swing so a single swing hits each Capi at most once.
    next_swing_id: u16,
    /// Current action state machine.
    action: ActiveAction,
    /// Current animation state for sprite lookup.
    pub anim: PlayerAnim,
    /// Frame counter driving 4-frame sprite animation cycling.
    anim_counter: u16,
}

impl Player {
    /// Creates a new player at pixel coordinates `(px, py)` with full hunger (`3`).
    #[must_use]
    pub fn new(px: i32, py: i32) -> Self {
        Self {
            pos: vec2(Num::new(px), Num::new(py)),
            facing: Facing::Front,
            weapon: Weapon::Blade,
            hunger: MAX_HUNGER,
            hunger_timer: HUNGER_DRAIN_FRAMES,
            has_raw_meat: false,
            has_cooked_meat: false,
            arrows: START_ARROWS,
            invuln_frames: 0,
            next_swing_id: 1,
            action: ActiveAction::None,
            anim: PlayerAnim::Idle,
            anim_counter: 0,
        }
    }

    /// Integer top-left pixel position of the `8x16` sprite in world coordinates.
    #[inline]
    #[must_use]
    pub fn pixel_pos(&self) -> Vector2D<i32> {
        vec2(self.pos.x.floor(), self.pos.y.floor())
    }

    /// World-space centre point of the player.
    #[inline]
    #[must_use]
    pub fn centre(&self) -> Vector2D<i32> {
        self.pixel_pos() + vec2(4, 10)
    }

    /// Compact `6x5` collision box at the player's boots/base so upper body overlaps canopies naturally.
    #[must_use]
    pub fn feet_rect_at(px: i32, py: i32) -> Rect<i32> {
        rect(vec2(px + 1, py + 10), vec2(6, 5))
    }

    /// Current world-space feet collision box.
    #[must_use]
    pub fn feet_rect(&self) -> Rect<i32> {
        let p = self.pixel_pos();
        Self::feet_rect_at(p.x, p.y)
    }

    /// Returns `(active_blade_rect, swing_id)` whenever the hunting blade's damaging arc is active.
    #[must_use]
    pub fn active_blade_hitbox(&self) -> Option<(Rect<i32>, u16)> {
        if let ActiveAction::BladeSwing { frames_left, hit_id } = self.action
            && (3..=13).contains(&frames_left)
        {
            let p = self.pixel_pos();
            let hitbox = match self.facing {
                Facing::Front => rect(vec2(p.x - 3, p.y + 11), vec2(14, 10)),
                Facing::Back => rect(vec2(p.x - 3, p.y - 6), vec2(14, 10)),
                Facing::Right => rect(vec2(p.x + 5, p.y + 3), vec2(11, 12)),
                Facing::Left => rect(vec2(p.x - 8, p.y + 3), vec2(11, 12)),
            };
            return Some((hitbox, hit_id));
        }
        None
    }

    /// Returns `true` while the player is actively sprinting (`Hold L` + moving).
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.anim, PlayerAnim::Run)
    }

    /// Moves the player along X and Y independently with solid world base-collision sliding.
    fn try_move(&mut self, world_seed: u32, dx: Num<i32, 8>, dy: Num<i32, 8>) {
        if dx != Num::new(0) {
            let cand_x = (self.pos.x + dx).clamp(Num::new(8), Num::new(WORLD_PX_W - 16));
            if !rect_collides_world(
                world_seed,
                Self::feet_rect_at(cand_x.floor(), self.pos.y.floor()),
            ) {
                self.pos.x = cand_x;
            }
        }
        if dy != Num::new(0) {
            let cand_y = (self.pos.y + dy).clamp(Num::new(8), Num::new(WORLD_PX_H - 24));
            if !rect_collides_world(
                world_seed,
                Self::feet_rect_at(self.pos.x.floor(), cand_y.floor()),
            ) {
                self.pos.y = cand_y;
            }
        }
    }

    /// Inflicts 1 hunger of fire damage when stepping onto a lit campfire, triggering `TakeDamage`.
    pub fn apply_fire_damage(
        &mut self,
        fire_centre: Vector2D<i32>,
        audio: &mut AudioManager,
        stats: &mut GameStats,
    ) {
        if self.invuln_frames > 0 {
            return;
        }
        self.hunger = self.hunger.saturating_sub(1);
        self.invuln_frames = 65;
        stats.damage_taken = stats.damage_taken.saturating_add(1);
        audio.play_sfx(SfxId::PlayerHurt);

        let diff = self.centre() - fire_centre;
        let kdx = if diff.x >= 0 { 1 } else { -1 };
        let kdy = if diff.y >= 0 { 1 } else { -1 };
        self.action = ActiveAction::Hurt {
            frames_left: 20,
            knock_dx: kdx,
            knock_dy: kdy,
        };
        self.anim = PlayerAnim::TakeDamage;
        self.anim_counter = 0;
    }

    /// Updates player timers, input actions, movement, campfire/meat interactions, and arrow spawns.
    pub fn update(
        &mut self,
        input: &ButtonController,
        world_seed: u32,
        campfires: &mut [Campfire],
        arrows_out: &mut Vec<ArrowProjectile>,
        audio: &mut AudioManager,
        stats: &mut GameStats,
    ) -> ContextPrompt {
        self.invuln_frames = self.invuln_frames.saturating_sub(1);

        // Passive hunger drain over time.
        if self.hunger_timer > 0 {
            self.hunger_timer -= 1;
        } else {
            self.hunger = self.hunger.saturating_sub(1);
            self.hunger_timer = HUNGER_DRAIN_FRAMES;
        }

        // `Button Select`: switch active weapon between Blade and Bow whenever not mid-swing.
        if input.is_just_pressed(Button::Select)
            && matches!(self.action, ActiveAction::None | ActiveAction::BowDraw { .. })
        {
            self.weapon = match self.weapon {
                Weapon::Blade => Weapon::Bow,
                Weapon::Bow => Weapon::Blade,
            };
            self.action = ActiveAction::None;
        }

        // Find nearest campfire within interaction distance (14 px from player centre).
        let p_centre = self.centre();
        let mut nearest_fire: Option<(usize, i32)> = None;
        for (idx, fire) in campfires.iter().enumerate() {
            let fc = fire.centre();
            let dist_man = (fc.x - p_centre.x).abs() + (fc.y - p_centre.y).abs();
            if dist_man <= 18
                && (nearest_fire.is_none() || dist_man < nearest_fire.unwrap().1)
            {
                nearest_fire = Some((idx, dist_man));
            }
        }

        // Compute what `Button R` can do right now for both execution and HUD prompt display.
        let prompt = if let Some((fidx, _)) = nearest_fire {
            let fire = &campfires[fidx];
            if !fire.lit {
                ContextPrompt::LightFire
            } else {
                match fire.meat {
                    FireMeatState::CookedReady if !self.has_cooked_meat => ContextPrompt::GrabCooked,
                    FireMeatState::Empty if self.has_raw_meat => ContextPrompt::CookMeat,
                    FireMeatState::Cooking { .. } => ContextPrompt::CookingInProgress,
                    _ if self.has_cooked_meat => ContextPrompt::EatMeat,
                    _ => ContextPrompt::None,
                }
            }
        } else if self.has_cooked_meat {
            ContextPrompt::EatMeat
        } else {
            ContextPrompt::None
        };

        // Advance active locked actions first.
        match self.action {
            ActiveAction::Hurt {
                frames_left,
                knock_dx,
                knock_dy,
            } => {
                self.anim = PlayerAnim::TakeDamage;
                self.anim_counter = self.anim_counter.wrapping_add(1);
                if frames_left > 12 {
                    self.try_move(world_seed, Num::new(knock_dx), Num::new(knock_dy));
                }
                if frames_left <= 1 {
                    self.action = ActiveAction::None;
                } else {
                    self.action = ActiveAction::Hurt {
                        frames_left: frames_left - 1,
                        knock_dx,
                        knock_dy,
                    };
                }
                return prompt;
            }
            ActiveAction::BladeSwing { frames_left, hit_id } => {
                self.anim = PlayerAnim::SwingBlade;
                self.anim_counter = (16 - frames_left) as u16;
                if frames_left <= 1 {
                    self.action = ActiveAction::None;
                } else {
                    self.action = ActiveAction::BladeSwing {
                        frames_left: frames_left - 1,
                        hit_id,
                    };
                }
                return prompt;
            }
            ActiveAction::BowRelease { frames_left } => {
                self.anim = PlayerAnim::ShootBow;
                self.anim_counter = 12;
                if frames_left <= 1 {
                    self.action = ActiveAction::None;
                } else {
                    self.action = ActiveAction::BowRelease {
                        frames_left: frames_left - 1,
                    };
                }
                return prompt;
            }
            ActiveAction::Eating { frames_left } => {
                self.anim = PlayerAnim::Eat;
                self.anim_counter = (24 - frames_left) as u16;
                if frames_left <= 1 {
                    self.action = ActiveAction::None;
                } else {
                    self.action = ActiveAction::Eating {
                        frames_left: frames_left - 1,
                    };
                }
                return prompt;
            }
            ActiveAction::LightingFire { frames_left } => {
                self.anim = PlayerAnim::StartFire;
                self.anim_counter = (24 - frames_left) as u16;
                if frames_left <= 1 {
                    self.action = ActiveAction::None;
                } else {
                    self.action = ActiveAction::LightingFire {
                        frames_left: frames_left - 1,
                    };
                }
                return prompt;
            }
            ActiveAction::BowDraw { draw_frames } => {
                // Allow aiming direction adjustments with D-Pad while holding bow!
                self.update_facing_from_dpad(input.x_tri(), input.y_tri());
                self.anim = PlayerAnim::ShootBow;
                let next_draw = draw_frames.saturating_add(1);
                self.anim_counter = (next_draw.min(18) / 6) as u16 * 4;

                // Release `A` after brief draw (or reach full draw hold at 26 frames) to shoot!
                let released_ready = !input.is_pressed(Button::A) && next_draw >= 8;
                let full_auto_loose = next_draw >= 26;

                if released_ready || full_auto_loose {
                    if self.arrows > 0 {
                        self.arrows -= 1;
                        stats.arrows_fired = stats.arrows_fired.saturating_add(1);
                        audio.play_sfx(SfxId::PlayerShootBow);
                        let p = self.pixel_pos();
                        arrows_out.push(ArrowProjectile::new(p.x, p.y + 4, self.facing));
                    }
                    self.action = ActiveAction::BowRelease { frames_left: 8 };
                } else if !input.is_pressed(Button::A) {
                    // Tapped too briefly without drawing bow fully -> cancel cleanly.
                    self.action = ActiveAction::None;
                } else {
                    self.action = ActiveAction::BowDraw {
                        draw_frames: next_draw,
                    };
                }
                return prompt;
            }
            ActiveAction::None => {}
        }

        // Handle contextual `Button R` (`eat/light fire/grab cooked food = Button R`).
        if input.is_just_pressed(Button::R) {
            match prompt {
                ContextPrompt::LightFire => {
                    if let Some((fidx, _)) = nearest_fire {
                        campfires[fidx].lit = true;
                        stats.fires_lit = stats.fires_lit.saturating_add(1);
                        audio.play_sfx(SfxId::PlayerStartFire);
                        self.action = ActiveAction::LightingFire { frames_left: 24 };
                        self.anim = PlayerAnim::StartFire;
                        self.anim_counter = 0;
                        return prompt;
                    }
                }
                ContextPrompt::CookMeat => {
                    if let Some((fidx, _)) = nearest_fire {
                        self.has_raw_meat = false;
                        campfires[fidx].meat = FireMeatState::Cooking {
                            frames_left: FIRE_COOK_FRAMES,
                        };
                        audio.play_sfx(SfxId::PlayerStartFire);
                        return ContextPrompt::CookingInProgress;
                    }
                }
                ContextPrompt::GrabCooked => {
                    if let Some((fidx, _)) = nearest_fire {
                        campfires[fidx].meat = FireMeatState::Empty;
                        self.has_cooked_meat = true;
                        audio.play_sfx(SfxId::PlayerWalk);
                        return ContextPrompt::EatMeat;
                    }
                }
                ContextPrompt::EatMeat => {
                    self.has_cooked_meat = false;
                    self.hunger = (self.hunger + 1).min(MAX_HUNGER);
                    self.hunger_timer = HUNGER_DRAIN_FRAMES;
                    stats.meat_eaten = stats.meat_eaten.saturating_add(1);
                    audio.play_sfx(SfxId::PlayerEat);
                    self.action = ActiveAction::Eating { frames_left: 24 };
                    self.anim = PlayerAnim::Eat;
                    self.anim_counter = 0;
                    return ContextPrompt::None;
                }
                ContextPrompt::CookingInProgress | ContextPrompt::None => {}
            }
        }

        // Handle weapon attack (`Button A` for Blade, `Hold A` for Bow).
        match self.weapon {
            Weapon::Blade => {
                if input.is_just_pressed(Button::A) {
                    let hit_id = self.next_swing_id;
                    self.next_swing_id = self.next_swing_id.wrapping_add(1).max(1);
                    audio.play_sfx(SfxId::PlayerSwingBlade);
                    self.action = ActiveAction::BladeSwing {
                        frames_left: 16,
                        hit_id,
                    };
                    self.anim = PlayerAnim::SwingBlade;
                    self.anim_counter = 0;
                    return prompt;
                }
            }
            Weapon::Bow => {
                if input.is_just_pressed(Button::A) && self.arrows > 0 {
                    self.action = ActiveAction::BowDraw { draw_frames: 1 };
                    self.anim = PlayerAnim::ShootBow;
                    self.anim_counter = 0;
                    return prompt;
                }
            }
        }

        // Standard movement (`D-Pad` + `Hold L` to run).
        let x_tri = input.x_tri();
        let y_tri = input.y_tri();
        let moving = x_tri != Tri::Zero || y_tri != Tri::Zero;

        if moving {
            self.update_facing_from_dpad(x_tri, y_tri);
            let running = input.is_pressed(Button::L);
            // Smooth fixed-point speeds: walk = 1.0 px/frame, run = 1.75 px/frame (diagonal scaled ~0.75x).
            let mut speed = if running {
                Num::<i32, 8>::new(7) / 4
            } else {
                Num::<i32, 8>::new(1)
            };
            if x_tri != Tri::Zero && y_tri != Tri::Zero {
                speed = (speed * 3) / 4;
            }
            let dx = speed * (x_tri as i32);
            let dy = speed * (y_tri as i32);
            self.try_move(world_seed, dx, dy);

            if running {
                self.anim = PlayerAnim::Run;
                audio.play_sfx(SfxId::PlayerRun);
            } else {
                self.anim = PlayerAnim::Walk;
                audio.play_sfx(SfxId::PlayerWalk);
            }
            self.anim_counter = self.anim_counter.wrapping_add(if running { 2 } else { 1 });
        } else {
            self.anim = PlayerAnim::Idle;
            self.anim_counter = self.anim_counter.wrapping_add(1);
        }

        prompt
    }

    /// Updates `self.facing` from current D-Pad axes.
    fn update_facing_from_dpad(&mut self, x_tri: Tri, y_tri: Tri) {
        match (x_tri, y_tri) {
            (Tri::Positive, _) => self.facing = Facing::Right,
            (Tri::Negative, _) => self.facing = Facing::Left,
            (Tri::Zero, Tri::Negative) => self.facing = Facing::Back,
            (Tri::Zero, Tri::Positive) => self.facing = Facing::Front,
            (Tri::Zero, Tri::Zero) => {}
        }
    }

    /// Resolves the current `(Tag, hflip)` pair for `(self.anim, self.facing)`.
    #[must_use]
    fn current_tag_and_flip(&self) -> (&'static Tag, bool) {
        let hflip = self.facing == Facing::Left;
        let tag = match (self.anim, self.facing) {
            (PlayerAnim::Idle, Facing::Front) => &sprites::IDLE_FRONT,
            (PlayerAnim::Idle, Facing::Back) => &sprites::IDLE_BACK,
            (PlayerAnim::Idle, _) => &sprites::IDLE_RIGHT,

            (PlayerAnim::Walk, Facing::Front) => &sprites::WALK_FRONT,
            (PlayerAnim::Walk, Facing::Back) => &sprites::WALK_BACK,
            (PlayerAnim::Walk, _) => &sprites::WALK_RIGHT,

            (PlayerAnim::Run, Facing::Front) => &sprites::RUN_FRONT,
            (PlayerAnim::Run, Facing::Back) => &sprites::RUN_BACK,
            (PlayerAnim::Run, _) => &sprites::RUN_RIGHT,

            (PlayerAnim::ShootBow, Facing::Front) => &sprites::SHOOT_BOW_FRONT,
            (PlayerAnim::ShootBow, Facing::Back) => &sprites::SHOOT_BOW_BACK,
            (PlayerAnim::ShootBow, _) => &sprites::SHOOT_BOW_RIGHT,

            (PlayerAnim::SwingBlade, Facing::Front) => &sprites::SWING_BLADE_FRONT,
            (PlayerAnim::SwingBlade, Facing::Back) => &sprites::SWING_BLADE_BACK,
            (PlayerAnim::SwingBlade, _) => &sprites::SWING_BLADE_RIGHT,

            (PlayerAnim::TakeDamage, Facing::Front) => &sprites::TAKE_DAMAGE_FRONT,
            (PlayerAnim::TakeDamage, Facing::Back) => &sprites::TAKE_DAMAGE_BACK,
            (PlayerAnim::TakeDamage, _) => &sprites::TAKE_DAMAGE_RIGHT,

            (PlayerAnim::Eat, Facing::Front) => &sprites::EAT_FRONT,
            (PlayerAnim::Eat, Facing::Back) => &sprites::EAT_BACK,
            (PlayerAnim::Eat, _) => &sprites::EAT_RIGHT,

            (PlayerAnim::StartFire, Facing::Front) => &sprites::START_FIRE_FRONT,
            (PlayerAnim::StartFire, Facing::Back) => &sprites::START_FIRE_BACK,
            (PlayerAnim::StartFire, _) => &sprites::START_FIRE_RIGHT,
        };
        (tag, hflip)
    }

    /// Renders the `8x16` player sprite (plus active blade slash arc gleam) relative to `cam`.
    pub fn show(&self, frame: &mut GraphicsFrame, cam: Vector2D<i32>) {
        // Subtle blink during post-damage invulnerability frames.
        if self.invuln_frames > 0 && (self.invuln_frames & 0b10) != 0 && self.anim != PlayerAnim::TakeDamage {
            return;
        }
        let screen_pos = self.pixel_pos() - cam;
        let (tag, hflip) = self.current_tag_and_flip();
        let frame_idx = match self.anim {
            PlayerAnim::Idle => (self.anim_counter as usize / 16) % 4,
            PlayerAnim::Walk | PlayerAnim::Run => (self.anim_counter as usize / 6) % 4,
            _ => (self.anim_counter as usize / 4) % 4,
        };

        // Render active melee blade slash gleam in front of the player during SwingBlade.
        if let ActiveAction::BladeSwing { frames_left, .. } = self.action {
            let slash_frame = ((16 - frames_left) as usize / 4).min(3);
            let (slash_sprite, offset, shflip, svflip): (&'static Sprite, Vector2D<i32>, bool, bool) =
                match self.facing {
                    Facing::Right => (
                        sprites::BLADE_SLASH_H.sprite(slash_frame),
                        vec2(7, 5),
                        false,
                        false,
                    ),
                    Facing::Left => (
                        sprites::BLADE_SLASH_H.sprite(slash_frame),
                        vec2(-7, 5),
                        true,
                        false,
                    ),
                    Facing::Front => (
                        sprites::BLADE_SLASH_V.sprite(slash_frame),
                        vec2(0, 14),
                        false,
                        false,
                    ),
                    Facing::Back => (
                        sprites::BLADE_SLASH_V.sprite(slash_frame),
                        vec2(0, -6),
                        false,
                        true,
                    ),
                };
            let mut slash_obj = Object::new(slash_sprite);
            slash_obj
                .set_pos(screen_pos + offset)
                .set_hflip(shflip)
                .set_vflip(svflip)
                .set_priority(Priority::P1)
                .show(frame);
        }

        let mut obj = Object::new(tag.sprite(frame_idx));
        obj.set_pos(screen_pos)
            .set_hflip(hflip)
            .set_priority(Priority::P1)
            .show(frame);
    }
}

// ---------------------------------------------------------------------------
// Peaceful Capybara ("Capi") Entity (8x8, 4 frames, 4 HP, drops 1 raw meat)
// ---------------------------------------------------------------------------

/// Behavioural state of a peaceful `8x8` Capi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapiState {
    /// Quietly grazing (`capi_idle`).
    Idle { timer: u16 },
    /// Gently wandering (`capi_walk`).
    Walk { timer: u16, dir: Vector2D<i32> },
    /// Startled and fleeing away from the hunter (`capi_run`).
    Flee { timer: u16 },
    /// Brief hurt flash after taking a hit (`capi_hurt`).
    Hurt { timer: u8 },
}

/// A peaceful `8x8` Capybara ("Capi") roaming meadows and forest clearings.
pub struct Capi {
    /// Subpixel top-left world position (`8x8`).
    pub pos: Vector2D<Num<i32, 8>>,
    /// Remaining hits before dropping 1 raw meat (`1..=4`).
    pub health: u8,
    /// `true` when facing left (`set_hflip(true)`), `false` when facing right.
    facing_left: bool,
    /// Current AI state.
    state: CapiState,
    /// Last melee `swing_id` that damaged this Capi (prevents multi-hitting in 1 swing).
    last_hit_swing: u16,
    /// Frame counter for 4-frame sprite animations.
    anim_tick: u16,
    /// Deterministic pseudo-random state per Capi.
    rng_seed: u32,
}

impl Capi {
    /// Spawns a healthy (`4` HP) peaceful Capi at pixel `(px, py)`.
    #[must_use]
    pub fn new(px: i32, py: i32, rng_seed: u32) -> Self {
        Self {
            pos: vec2(Num::new(px), Num::new(py)),
            health: CAPI_MAX_HEALTH,
            facing_left: (rng_seed & 1) != 0,
            state: CapiState::Idle {
                timer: 40 + ((rng_seed & 0x3F) as u16),
            },
            last_hit_swing: 0,
            anim_tick: (rng_seed & 0x1F) as u16,
            rng_seed,
        }
    }

    /// Integer top-left pixel coordinate.
    #[inline]
    #[must_use]
    pub fn pixel_pos(&self) -> Vector2D<i32> {
        vec2(self.pos.x.floor(), self.pos.y.floor())
    }

    /// World-space `8x8` hurtbox for blade and arrow hits.
    #[must_use]
    pub fn hitbox(&self) -> Rect<i32> {
        rect(self.pixel_pos(), vec2(8, 8))
    }

    /// Compact `6x5` base collision box for world obstacle checks.
    #[must_use]
    fn base_rect_at(px: i32, py: i32) -> Rect<i32> {
        rect(vec2(px + 1, py + 3), vec2(6, 5))
    }

    /// Applies 1 hit from the player's blade or arrow. Returns `true` if defeated (`health == 0`).
    pub fn take_hit(&mut self, swing_id_opt: Option<u16>, audio: &mut AudioManager) -> bool {
        if let Some(sid) = swing_id_opt {
            if self.last_hit_swing == sid {
                return false;
            }
            self.last_hit_swing = sid;
        }
        self.health = self.health.saturating_sub(1);
        audio.play_sfx(SfxId::CapiHurt);
        self.state = CapiState::Hurt { timer: 16 };
        self.anim_tick = 0;
        self.health == 0
    }

    /// Advances peaceful grazing, startling/fleeing from the hunter, and movement collisions.
    pub fn update(
        &mut self,
        world_seed: u32,
        player_centre: Vector2D<i32>,
        player_running: bool,
        cam: Vector2D<i32>,
        audio: &mut AudioManager,
    ) {
        self.anim_tick = self.anim_tick.wrapping_add(1);
        self.rng_seed = hash_coords(self.rng_seed, self.anim_tick as i32, self.health as i32);

        let my_centre = self.pixel_pos() + vec2(4, 4);
        let delta_from_player = my_centre - player_centre;
        let dist_man = delta_from_player.x.abs() + delta_from_player.y.abs();
        let on_screen = {
            let sp = self.pixel_pos() - cam;
            sp.x >= -8 && sp.x <= 240 && sp.y >= 16 && sp.y <= 160
        };

        // Startle radius: sprinting hunter scares Capis from much farther away than stealthy walking!
        let startle_dist = if player_running { 82 } else { 36 };
        if dist_man < startle_dist && !matches!(self.state, CapiState::Hurt { .. }) {
            self.state = CapiState::Flee { timer: 95 };
        }

        match self.state {
            CapiState::Hurt { timer } => {
                // Slight recoil away from player on initial hurt frames.
                if timer > 10 {
                    let rx = if delta_from_player.x >= 0 { 1 } else { -1 };
                    let ry = if delta_from_player.y >= 0 { 1 } else { -1 };
                    self.step_move(world_seed, Num::new(rx), Num::new(ry));
                }
                if timer <= 1 {
                    self.state = CapiState::Flee { timer: 140 };
                } else {
                    self.state = CapiState::Hurt { timer: timer - 1 };
                }
            }
            CapiState::Flee { timer } => {
                let fx = if delta_from_player.x >= 0 { 1 } else { -1 };
                let fy = if delta_from_player.y.abs() > 6 {
                    if delta_from_player.y >= 0 { 1 } else { -1 }
                } else {
                    0
                };
                self.facing_left = fx < 0;
                // Flee speed ~1.25 px/frame (slower than player's 1.75 run so a determined hunter can catch up!).
                let speed = Num::<i32, 8>::new(5) / 4;
                if !self.step_move(world_seed, speed * fx, speed * fy) {
                    // Slide perpendicular around trees/rocks if directly blocked.
                    let _ = self.step_move(world_seed, Num::new(0), speed * (if (self.rng_seed & 2) == 0 { 1 } else { -1 }));
                }
                if on_screen && (self.anim_tick & 0x0F) == 0 {
                    audio.play_sfx_scaled(SfxId::CapiRun, 8, 0);
                }
                if timer <= 1 {
                    self.state = CapiState::Idle { timer: 60 };
                } else {
                    self.state = CapiState::Flee { timer: timer - 1 };
                }
            }
            CapiState::Idle { timer } => {
                if timer <= 1 {
                    let dirs = [vec2(1, 0), vec2(-1, 0), vec2(0, 1), vec2(0, -1)];
                    let d = dirs[(self.rng_seed as usize) & 3];
                    if d.x != 0 {
                        self.facing_left = d.x < 0;
                    }
                    self.state = CapiState::Walk {
                        timer: 45 + ((self.rng_seed as u16) & 0x3F),
                        dir: d,
                    };
                } else {
                    self.state = CapiState::Idle { timer: timer - 1 };
                }
            }
            CapiState::Walk { timer, dir } => {
                let speed = Num::<i32, 8>::new(1) / 2;
                if !self.step_move(world_seed, speed * dir.x, speed * dir.y) || timer <= 1 {
                    self.state = CapiState::Idle {
                        timer: 50 + ((self.rng_seed as u16) & 0x3F),
                    };
                } else {
                    if on_screen && (self.anim_tick & 0x1F) == 0 {
                        audio.play_sfx_scaled(SfxId::CapiWalk, 6, 0);
                    }
                    self.state = CapiState::Walk {
                        timer: timer - 1,
                        dir,
                    };
                }
            }
        }
    }

    /// Moves Capi while respecting solid base collision boxes. Returns `true` if any axis moved.
    fn step_move(&mut self, world_seed: u32, dx: Num<i32, 8>, dy: Num<i32, 8>) -> bool {
        let mut moved = false;
        if dx != Num::new(0) {
            let cx = (self.pos.x + dx).clamp(Num::new(24), Num::new(WORLD_PX_W - 32));
            if !rect_collides_world(world_seed, Self::base_rect_at(cx.floor(), self.pos.y.floor())) {
                self.pos.x = cx;
                moved = true;
            }
        }
        if dy != Num::new(0) {
            let cy = (self.pos.y + dy).clamp(Num::new(24), Num::new(WORLD_PX_H - 32));
            if !rect_collides_world(world_seed, Self::base_rect_at(self.pos.x.floor(), cy.floor())) {
                self.pos.y = cy;
                moved = true;
            }
        }
        moved
    }

    /// Renders the `8x8` Capi if within the visible `240x160` viewport.
    pub fn show(&self, frame: &mut GraphicsFrame, cam: Vector2D<i32>) {
        let sp = self.pixel_pos() - cam;
        if sp.x < -8 || sp.x > 240 || sp.y < 10 || sp.y > 160 {
            return;
        }
        let (tag, div) = match self.state {
            CapiState::Idle { .. } => (&sprites::CAPI_IDLE, 18usize),
            CapiState::Walk { .. } => (&sprites::CAPI_WALK, 10usize),
            CapiState::Flee { .. } => (&sprites::CAPI_RUN, 5usize),
            CapiState::Hurt { .. } => (&sprites::CAPI_HURT, 4usize),
        };
        let fidx = (self.anim_tick as usize / div) % 4;
        let mut obj = Object::new(tag.sprite(fidx));
        obj.set_pos(sp)
            .set_hflip(self.facing_left)
            .set_priority(Priority::P1)
            .show(frame);
    }
}

// ---------------------------------------------------------------------------
// Campfires (8x8, lit/unlit, holds 1 meat, cooks in ~10s, burns if stepped on)
// ---------------------------------------------------------------------------

/// Meat cooking state for a single `8x8` campfire (`can hold only 1 meat at a time`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireMeatState {
    /// No meat currently on this campfire.
    Empty,
    /// Raw meat is roasting (`frames_left` counts down from `600` = ~10s).
    Cooking { frames_left: u16 },
    /// Meat has finished cooking and is ready to grab with `Button R`.
    CookedReady,
}

/// An `8x8` stone campfire pit in the world.
#[derive(Debug, Clone, Copy)]
pub struct Campfire {
    /// World tile X coordinate (`0..1024`).
    pub tile_x: i32,
    /// World tile Y coordinate (`0..1024`).
    pub tile_y: i32,
    /// `true` when lit, `false` when unlit.
    pub lit: bool,
    /// Current meat slot state (holds max 1 meat).
    pub meat: FireMeatState,
}

impl Campfire {
    /// Creates an unlit, empty campfire at `(tile_x, tile_y)`.
    #[must_use]
    pub const fn new_unlit(tile_x: i32, tile_y: i32) -> Self {
        Self {
            tile_x,
            tile_y,
            lit: false,
            meat: FireMeatState::Empty,
        }
    }

    /// World pixel top-left coordinate.
    #[inline]
    #[must_use]
    pub const fn pixel_pos(&self) -> Vector2D<i32> {
        vec2(self.tile_x * TILE_PX, self.tile_y * TILE_PX)
    }

    /// World pixel centre coordinate.
    #[inline]
    #[must_use]
    pub const fn centre(&self) -> Vector2D<i32> {
        vec2(self.tile_x * TILE_PX + 4, self.tile_y * TILE_PX + 4)
    }

    /// Inner `6x5` flame hazard hitbox — stepping into a lit fire burns the hunter!
    #[must_use]
    pub fn hazard_rect(&self) -> Rect<i32> {
        let p = self.pixel_pos();
        rect(vec2(p.x + 1, p.y + 2), vec2(6, 5))
    }

    /// Advances the ~10-second meat cooking timer when lit.
    pub fn tick(&mut self, stats: &mut GameStats) {
        if self.lit
            && let FireMeatState::Cooking { frames_left } = self.meat
        {
            if frames_left <= 1 {
                self.meat = FireMeatState::CookedReady;
                stats.meat_cooked = stats.meat_cooked.saturating_add(1);
            } else {
                self.meat = FireMeatState::Cooking {
                    frames_left: frames_left - 1,
                };
            }
        }
    }

    /// Converts a modified campfire (`lit` or holding meat) into a compact [`SavedCampfire`].
    #[must_use]
    pub const fn to_saved(&self) -> Option<SavedCampfire> {
        if !self.lit && matches!(self.meat, FireMeatState::Empty) {
            return None;
        }
        let (meat_state, cook_frames_left) = match self.meat {
            FireMeatState::Empty => (0, 0),
            FireMeatState::Cooking { frames_left } => (1, frames_left),
            FireMeatState::CookedReady => (2, 0),
        };
        Some(SavedCampfire {
            tile_x: self.tile_x as u16,
            tile_y: self.tile_y as u16,
            lit: self.lit,
            meat_state,
            cook_frames_left,
        })
    }

    /// Reconstructs a [`Campfire`] from SRAM save data.
    #[must_use]
    pub const fn from_saved(s: SavedCampfire) -> Self {
        let meat = match s.meat_state {
            1 => FireMeatState::Cooking {
                frames_left: s.cook_frames_left,
            },
            2 => FireMeatState::CookedReady,
            _ => FireMeatState::Empty,
        };
        Self {
            tile_x: s.tile_x as i32,
            tile_y: s.tile_y as i32,
            lit: s.lit,
            meat,
        }
    }

    /// Renders the `8x8` campfire (unlit logs or 4-frame flickering flames) and any meat on it.
    pub fn show(&self, frame: &mut GraphicsFrame, cam: Vector2D<i32>, global_tick: u32) {
        let sp = self.pixel_pos() - cam;
        if sp.x < -8 || sp.x > 240 || sp.y < 8 || sp.y > 160 {
            return;
        }
        // Render cooking / ready meat floating just above the campfire flames!
        let bob = ((global_tick / 14) & 1) as i32;
        match self.meat {
            FireMeatState::Empty => {}
            FireMeatState::Cooking { frames_left } => {
                let mut meat_obj = Object::new(sprites::MEAT_RAW.sprite(0));
                meat_obj
                    .set_pos(sp + vec2(0, -6 - bob))
                    .set_priority(Priority::P1)
                    .show(frame);
                // Show a rising spark during the final 3 seconds so progress is visually clear!
                if frames_left < 180 && (frames_left & 0b100) == 0 {
                    let mut spark = Object::new(sprites::HUD_ICONS.sprite(hud_icon::COOK_SPARK));
                    spark
                        .set_pos(sp + vec2(0, -12))
                        .set_priority(Priority::P1)
                        .show(frame);
                }
            }
            FireMeatState::CookedReady => {
                let mut cooked_obj = Object::new(sprites::MEAT_COOKED.sprite(0));
                cooked_obj
                    .set_pos(sp + vec2(0, -7 - bob))
                    .set_priority(Priority::P1)
                    .show(frame);
            }
        }

        let tag = if self.lit {
            &sprites::FIRE_LIT
        } else {
            &sprites::FIRE_UNLIT
        };
        let fidx = ((global_tick as usize + (self.tile_x as usize * 3)) / 6) % 4;
        let mut fire_obj = Object::new(tag.sprite(fidx));
        fire_obj
            .set_pos(sp)
            .set_priority(Priority::P1)
            .show(frame);
    }
}

/// Ensures campfires from all `16x16` chunks near `player_pos` are loaded into `fires`.
pub fn sync_nearby_campfires(world_seed: u32, player_pos: Vector2D<i32>, fires: &mut Vec<Campfire>) {
    let pcx = (player_pos.x / TILE_PX) >> 4;
    let pcy = (player_pos.y / TILE_PX) >> 4;

    for cy in (pcy - 2)..=(pcy + 2) {
        for cx in (pcx - 2)..=(pcx + 2) {
            if let Some((tx, ty)) = chunk_campfire_tile(world_seed, cx, cy)
                && !fires.iter().any(|f| f.tile_x == tx && f.tile_y == ty)
            {
                // Cap total tracked campfires only by pruning far-away unlit empty pits.
                if fires.len() >= 48 {
                    fires.retain(|f| {
                        f.lit
                            || !matches!(f.meat, FireMeatState::Empty)
                            || ((f.tile_x >> 4) - pcx).abs() <= 2
                                && ((f.tile_y >> 4) - pcy).abs() <= 2
                    });
                }
                fires.push(Campfire::new_unlit(tx, ty));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Projectiles (Bow Arrows) & World Pickups (Dropped Raw Meat & Arrow Bundles)
// ---------------------------------------------------------------------------

/// An `8x8` arrow loosed from the player's hunting bow.
pub struct ArrowProjectile {
    /// Subpixel position.
    pub pos: Vector2D<Num<i32, 8>>,
    /// Velocity per frame (`~3.0 px/frame`).
    vel: Vector2D<Num<i32, 8>>,
    /// Travel direction for sprite orientation.
    facing: Facing,
    /// Remaining flight frames before falling to the ground.
    pub life: u8,
}

impl ArrowProjectile {
    /// Spawns a flying arrow moving in `facing`.
    #[must_use]
    pub fn new(px: i32, py: i32, facing: Facing) -> Self {
        let u = facing.unit_vec();
        let speed = Num::<i32, 8>::new(3);
        Self {
            pos: vec2(Num::new(px), Num::new(py)),
            vel: vec2(speed * u.x, speed * u.y),
            facing,
            life: 52,
        }
    }

    /// World-space `6x6` hitbox of the arrow tip.
    #[must_use]
    pub fn hitbox(&self) -> Rect<i32> {
        rect(
            vec2(self.pos.x.floor() + 1, self.pos.y.floor() + 1),
            vec2(6, 6),
        )
    }

    /// Steps the arrow forward; returns `false` if expired or stopped by a solid rock/tree base.
    pub fn update(&mut self, world_seed: u32) -> bool {
        if self.life == 0 {
            return false;
        }
        self.life -= 1;
        self.pos += self.vel;
        !rect_collides_world(world_seed, self.hitbox())
    }

    /// Renders the flying arrow sprite.
    pub fn show(&self, frame: &mut GraphicsFrame, cam: Vector2D<i32>) {
        let sp = vec2(self.pos.x.floor(), self.pos.y.floor()) - cam;
        if sp.x < -8 || sp.x > 240 || sp.y < 10 || sp.y > 160 {
            return;
        }
        let (spr, hflip, vflip) = match self.facing {
            Facing::Right => (sprites::ARROW_H.sprite(0), false, false),
            Facing::Left => (sprites::ARROW_H.sprite(0), true, false),
            Facing::Front => (sprites::ARROW_V.sprite(0), false, false),
            Facing::Back => (sprites::ARROW_V.sprite(0), false, true),
        };
        let mut obj = Object::new(spr);
        obj.set_pos(sp)
            .set_hflip(hflip)
            .set_vflip(vflip)
            .set_priority(Priority::P1)
            .show(frame);
    }
}

/// Kind of collectible `8x8` item lying in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// Raw meat dropped by a hunted Capi (`drops = 1 meat`).
    RawMeat,
    /// Bundle of `+3` arrows found in forests and meadows (`chunk_key` prevents instant respawn).
    ArrowBundle { chunk_x: i16, chunk_y: i16 },
}

/// An `8x8` item lying on the ground waiting to be picked up.
pub struct WorldPickup {
    /// Pixel coordinates in world space.
    pub pos: Vector2D<i32>,
    /// Item variant.
    pub kind: PickupKind,
}

impl WorldPickup {
    /// Hitbox for player walk-over collection.
    #[must_use]
    pub fn hitbox(&self) -> Rect<i32> {
        rect(self.pos, vec2(8, 8))
    }

    /// Renders the `8x8` ground item with a gentle bob.
    pub fn show(&self, frame: &mut GraphicsFrame, cam: Vector2D<i32>, tick: u32) {
        let sp = self.pos - cam;
        if sp.x < -8 || sp.x > 240 || sp.y < 10 || sp.y > 160 {
            return;
        }
        let bob = ((tick / 20) & 1) as i32;
        let spr = match self.kind {
            PickupKind::RawMeat => sprites::MEAT_RAW.sprite(0),
            PickupKind::ArrowBundle { .. } => sprites::ARROW_PICKUP.sprite(0),
        };
        let mut obj = Object::new(spr);
        obj.set_pos(sp - vec2(0, bob))
            .set_priority(Priority::P1)
            .show(frame);
    }
}

/// Spawns forageable arrow bundles from nearby chunks if not recently collected.
pub fn sync_nearby_arrow_pickups(
    world_seed: u32,
    player_pos: Vector2D<i32>,
    pickups: &mut Vec<WorldPickup>,
    collected_chunks: &[(i16, i16)],
) {
    let pcx = (player_pos.x / TILE_PX) >> 4;
    let pcy = (player_pos.y / TILE_PX) >> 4;

    // Prune far-away uncollected arrow pickups to bound entity count.
    pickups.retain(|p| match p.kind {
        PickupKind::RawMeat => (p.pos.x - player_pos.x).abs() < 640 && (p.pos.y - player_pos.y).abs() < 640,
        PickupKind::ArrowBundle { chunk_x, chunk_y } => {
            (chunk_x as i32 - pcx).abs() <= 2 && (chunk_y as i32 - pcy).abs() <= 2
        }
    });

    for cy in (pcy - 2)..=(pcy + 2) {
        for cx in (pcx - 2)..=(pcx + 2) {
            let key = (cx as i16, cy as i16);
            if collected_chunks.contains(&key) {
                continue;
            }
            let already = pickups.iter().any(|p| {
                matches!(p.kind, PickupKind::ArrowBundle { chunk_x, chunk_y } if chunk_x == key.0 && chunk_y == key.1)
            });
            if !already
                && let Some((tx, ty)) = chunk_arrow_pickup_tile(world_seed, cx, cy)
            {
                pickups.push(WorldPickup {
                    pos: vec2(tx * TILE_PX, ty * TILE_PX),
                    kind: PickupKind::ArrowBundle {
                        chunk_x: key.0,
                        chunk_y: key.1,
                    },
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Top-of-Screen Gameplay HUD Sprites
// ---------------------------------------------------------------------------

/// Renders the sprite icons along the top `16px` HUD bar:
/// - 3 Hunger pips (`HUNGER_FULL` / `HUNGER_EMPTY`)
/// - Active Weapon icon (`WEAPON_BLADE` / `WEAPON_BOW`)
/// - Carried Raw Meat (`MEAT_RAW`) & Cooked Meat (`MEAT_COOKED`) slots
pub fn show_hud_sprites(frame: &mut GraphicsFrame, player: &Player) {
    // Hunger = 3 drumsticks at top-left (x = 4, 14, 24)
    for i in 0..MAX_HUNGER {
        let icon = if i < player.hunger {
            hud_icon::HUNGER_FULL
        } else {
            hud_icon::HUNGER_EMPTY
        };
        let mut obj = Object::new(sprites::HUD_ICONS.sprite(icon));
        obj.set_pos(vec2(4 + (i as i32) * 10, 4))
            .set_priority(Priority::P0)
            .show(frame);
    }

    // Active Weapon icon at x = 42
    let w_icon = match player.weapon {
        Weapon::Blade => hud_icon::WEAPON_BLADE,
        Weapon::Bow => hud_icon::WEAPON_BOW,
    };
    let mut w_obj = Object::new(sprites::HUD_ICONS.sprite(w_icon));
    w_obj
        .set_pos(vec2(42, 4))
        .set_priority(Priority::P0)
        .show(frame);

    // Carried Raw Meat icon (x = 96) and Cooked Meat icon (x = 110) when held
    if player.has_raw_meat {
        let mut raw_obj = Object::new(sprites::HUD_ICONS.sprite(hud_icon::MEAT_RAW));
        raw_obj
            .set_pos(vec2(96, 4))
            .set_priority(Priority::P0)
            .show(frame);
    }
    if player.has_cooked_meat {
        let mut cooked_obj = Object::new(sprites::HUD_ICONS.sprite(hud_icon::MEAT_COOKED));
        cooked_obj
            .set_pos(vec2(110, 4))
            .set_priority(Priority::P0)
            .show(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn campfire_single_meat_cooks_in_ten_seconds(_gba: &mut agb::Gba) {
        let mut fire = Campfire::new_unlit(516, 512);
        let mut stats = GameStats::default();
        fire.meat = FireMeatState::Cooking {
            frames_left: FIRE_COOK_FRAMES,
        };
        // Unlit fire must not advance cooking timer.
        fire.tick(&mut stats);
        assert_eq!(
            fire.meat,
            FireMeatState::Cooking {
                frames_left: FIRE_COOK_FRAMES
            }
        );

        // Lit fire finishes cooking after exactly FIRE_COOK_FRAMES (600 frames = ~10s).
        fire.lit = true;
        for _ in 0..(FIRE_COOK_FRAMES - 1) {
            fire.tick(&mut stats);
            assert!(matches!(fire.meat, FireMeatState::Cooking { .. }));
        }
        fire.tick(&mut stats);
        assert_eq!(fire.meat, FireMeatState::CookedReady);
        assert_eq!(stats.meat_cooked, 1);
    }

    #[test_case]
    fn player_defaults_and_capi_health(gba: &mut agb::Gba) {
        let player = Player::new(512 * 8, 512 * 8);
        assert_eq!(player.hunger, 3);
        assert!(!player.has_raw_meat);
        assert!(!player.has_cooked_meat);

        let mut audio = AudioManager::new(&mut gba.mixer, crate::save::AudioOptions::default());
        let mut capi = Capi::new(520 * 8, 512 * 8, 42);
        assert_eq!(capi.health, 4);
        for hit in 1..=3 {
            assert!(!capi.take_hit(Some(hit), &mut audio));
        }
        assert!(capi.take_hit(Some(4), &mut audio));
        assert_eq!(capi.health, 0);
    }
}
