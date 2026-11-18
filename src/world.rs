//! Deterministic `1024x1024` tile world generator, base-collision queries, and
//! [`InfiniteScrolledMap`] streaming renderer for **CapiHunt**.
//!
//! Because the Game Boy Advance only has 256 KiB of EWRAM, storing a `1024x1024`
//! tile array in RAM would consume 1 MiB and exceed hardware memory. Instead,
//! [`WorldMap`] evaluates biomes (simple forests and open meadows), trees, rocks,
//! scattered campfires, and arrow bundle spawn sites procedurally in `O(1)` time
//! from integer coordinates `(tx, ty)` with **zero map RAM overhead**.

use agb::{
    display::{
        GraphicsFrame, Priority,
        tiled::{
            InfiniteScrolledMap, PartialUpdateStatus, RegularBackground, RegularBackgroundSize,
            TileFormat, TileSet, TileSetting,
        },
    },
    fixnum::{Rect, Vector2D, rect, vec2},
};

use crate::assets::backgrounds;

/// Total world width in `8x8` tiles, as specified in `docs/INTENT.md`.
pub const WORLD_TILES_W: i32 = 1024;
/// Total world height in `8x8` tiles, as specified in `docs/INTENT.md`.
pub const WORLD_TILES_H: i32 = 1024;
/// Width and height of a single background tile in pixels.
pub const TILE_PX: i32 = 8;
/// Total world width in pixels (`1024 * 8 = 8192`).
pub const WORLD_PX_W: i32 = WORLD_TILES_W * TILE_PX;
/// Total world height in pixels (`1024 * 8 = 8192`).
pub const WORLD_PX_H: i32 = WORLD_TILES_H * TILE_PX;
/// Thickness of the impassable cliff/hedge border framing the `1024x1024` world.
pub const BORDER_TILES: i32 = 3;

/// Default spawn tile X at the centre meadow clearing of the `1024x1024` world.
pub const START_TILE_X: i32 = 512;
/// Default spawn tile Y at the centre meadow clearing of the `1024x1024` world.
pub const START_TILE_Y: i32 = 512;

/// Tile indices inside `gfx/world/world_tiles.png` (`8x4` = 32 tiles).
pub mod tile_id {
    /// Plain meadow grass tile A.
    pub const MEADOW_A: usize = 0;
    /// Subtle meadow grass blades tile B.
    pub const MEADOW_B: usize = 1;
    /// Yellow wildflower meadow tile.
    pub const FLOWER_YELLOW: usize = 2;
    /// White wildflower meadow tile.
    pub const FLOWER_WHITE: usize = 3;
    /// Lush clover meadow tile.
    pub const CLOVER: usize = 4;
    /// Shaded forest moss floor tile A.
    pub const FOREST_FLOOR_A: usize = 5;
    /// Shaded forest moss floor tile B with fallen leaf.
    pub const FOREST_FLOOR_B: usize = 6;
    /// Packed dirt trail / campfire clearing tile.
    pub const DIRT_CLEARING: usize = 7;
    /// Forest tree upper canopy top-left (walkable behind canopy).
    pub const TREE_CANOPY_TL: usize = 8;
    /// Forest tree upper canopy top-right (walkable behind canopy).
    pub const TREE_CANOPY_TR: usize = 9;
    /// Forest tree trunk base bottom-left (**solid collision box on base**).
    pub const TREE_BASE_BL: usize = 10;
    /// Forest tree trunk base bottom-right (**solid collision box on base**).
    pub const TREE_BASE_BR: usize = 11;
    /// Small mossy meadow rock (**solid collision box on base**).
    pub const ROCK_SMALL: usize = 12;
    /// Tall forest boulder upper peak (walkable behind top).
    pub const BOULDER_TOP: usize = 13;
    /// Tall forest boulder base (**solid collision box on base**).
    pub const BOULDER_BASE: usize = 14;
    /// Impassable outer world border hedge/cliff wall.
    pub const BORDER_WALL: usize = 15;
    /// Dark slate HUD bar tile with bottom wood trim.
    pub const HUD_BAR: usize = 16;
    /// Solid dark slate panel tile.
    pub const PANEL_DARK: usize = 17;
}

/// Biome classification for ambient audio and entity spawning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    /// Open sunlit grassland with wildflowers and grazing Capis.
    Meadow,
    /// Denser woodland with multi-tile trees, boulders, and leaf rustles.
    Forest,
}

/// Fast deterministic 32-bit coordinate hash (avoids slow division on ARM7TDMI).
#[inline]
#[must_use]
pub const fn hash_coords(seed: u32, x: i32, y: i32) -> u32 {
    let mut h = seed
        .wrapping_add((x as u32).wrapping_mul(0x9E37_79B1))
        .wrapping_add((y as u32).wrapping_mul(0x85EB_CA77));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Returns `true` if `(tx, ty)` lies inside the guaranteed-clear starter camp clearing.
#[inline]
#[must_use]
pub const fn is_starter_clearing(tx: i32, ty: i32) -> bool {
    let dx = (tx - START_TILE_X).abs();
    let dy = (ty - START_TILE_Y).abs();
    dx <= 8 && dy <= 7
}

/// Evaluates the biome at tile `(tx, ty)` using smooth coarse region hashing.
#[must_use]
pub const fn biome_at(seed: u32, tx: i32, ty: i32) -> Biome {
    if is_starter_clearing(tx, ty) {
        return Biome::Meadow;
    }
    // Blend two coarse grid scales (32-tile and 16-tile) for natural forest/meadow patches.
    let macro_region = hash_coords(seed ^ 0xA5A5_5A5A, tx >> 5, ty >> 5) & 0xFF;
    let meso_region = hash_coords(seed ^ 0x3C3C_C3C3, tx >> 4, ty >> 4) & 0x7F;
    if macro_region + meso_region > 195 {
        Biome::Forest
    } else {
        Biome::Meadow
    }
}

/// Deterministically computes the `16x16` chunk's campfire tile coordinate, if that chunk hosts one.
#[must_use]
pub const fn chunk_campfire_tile(seed: u32, chunk_x: i32, chunk_y: i32) -> Option<(i32, i32)> {
    // Starter chunk (32, 32) -> (512..528, 512..528) always has an unlit campfire near spawn!
    if chunk_x == (START_TILE_X >> 4) && chunk_y == (START_TILE_Y >> 4) {
        return Some((START_TILE_X + 4, START_TILE_Y));
    }
    // Keep campfires inside playable bounds away from the 3-tile border.
    if chunk_x <= 0 || chunk_y <= 0 || chunk_x >= 63 || chunk_y >= 63 {
        return None;
    }
    let h = hash_coords(seed ^ 0xF1AE_C491, chunk_x, chunk_y);
    // Roughly 1 in 3 chunks (16x16 tiles = 128x128 px) contains a stone campfire ring.
    if (h & 0xFF) < 88 {
        let ox = 4 + (((h >> 8) & 0x07) as i32); // 4..=11 (even/safe interior)
        let oy = 4 + (((h >> 12) & 0x07) as i32);
        Some(((chunk_x << 4) + ox, (chunk_y << 4) + oy))
    } else {
        None
    }
}

/// Returns `true` if tile `(tx, ty)` is within 1 tile of a deterministic campfire clearing.
#[inline]
#[must_use]
const fn is_near_campfire_site(seed: u32, tx: i32, ty: i32) -> bool {
    if let Some((fx, fy)) = chunk_campfire_tile(seed, tx >> 4, ty >> 4) {
        (tx - fx).abs() <= 2 && (ty - fy).abs() <= 2
    } else {
        false
    }
}

/// Deterministically computes the `16x16` chunk's forageable arrow pickup tile, if present.
#[must_use]
pub fn chunk_arrow_pickup_tile(seed: u32, chunk_x: i32, chunk_y: i32) -> Option<(i32, i32)> {
    if chunk_x == (START_TILE_X >> 4) - 1 && chunk_y == (START_TILE_Y >> 4) {
        return Some((START_TILE_X - 4, START_TILE_Y + 2));
    }
    if chunk_x <= 0 || chunk_y <= 0 || chunk_x >= 63 || chunk_y >= 63 {
        return None;
    }
    let h = hash_coords(seed ^ 0xA880_B017, chunk_x, chunk_y);
    if (h & 0xFF) < 110 {
        let tx = (chunk_x << 4) + 2 + (((h >> 8) % 12) as i32);
        let ty = (chunk_y << 4) + 2 + (((h >> 14) % 12) as i32);
        if !tile_has_base_collision(tile_index_at(seed, tx, ty)) {
            return Some((tx, ty));
        }
    }
    None
}

/// Computes the exact tile index (`0..16`) at world tile `(tx, ty)`.
#[must_use]
pub const fn tile_index_at(seed: u32, tx: i32, ty: i32) -> usize {
    // Everything on or outside the 1024x1024 border is solid border wall.
    if tx < BORDER_TILES
        || ty < BORDER_TILES
        || tx >= WORLD_TILES_W - BORDER_TILES
        || ty >= WORLD_TILES_H - BORDER_TILES
    {
        return tile_id::BORDER_WALL;
    }

    // Small dirt clearing immediately around campfires and the starter camp.
    if is_near_campfire_site(seed, tx, ty) {
        let d = hash_coords(seed, tx, ty) & 0x07;
        return if d == 0 {
            tile_id::DIRT_CLEARING
        } else if d < 4 {
            tile_id::MEADOW_A
        } else {
            tile_id::MEADOW_B
        };
    }

    if is_starter_clearing(tx, ty) {
        return match hash_coords(seed, tx, ty) & 0x0F {
            0 => tile_id::FLOWER_YELLOW,
            1 => tile_id::FLOWER_WHITE,
            2..=4 => tile_id::CLOVER,
            5..=9 => tile_id::MEADOW_B,
            _ => tile_id::MEADOW_A,
        };
    }

    let biome = biome_at(seed, tx, ty);

    // Evaluate 2x2 cell for multi-tile Forest Trees (canopy top row, solid trunk bottom row)
    // and 1x2 Boulders so multi-tile props never overlap each other.
    let cell_x = tx >> 1;
    let cell_y = ty >> 1;
    let sub_x = tx & 1;
    let sub_y = ty & 1;

    // Avoid spawning obstacles directly adjacent to the border wall or campfire clearings.
    let cell_clear = !is_near_campfire_site(seed, cell_x << 1, cell_y << 1)
        && !is_near_campfire_site(seed, (cell_x << 1) + 1, (cell_y << 1) + 1)
        && !is_starter_clearing(cell_x << 1, cell_y << 1);

    if cell_clear {
        let cell_h = hash_coords(seed ^ 0x77EE_1234, cell_x, cell_y) & 0xFF;
        match biome {
            Biome::Forest => {
                // Dense trees in forest biome (~22% of 2x2 cells)
                if cell_h < 56 {
                    return match (sub_x, sub_y) {
                        (0, 0) => tile_id::TREE_CANOPY_TL,
                        (1, 0) => tile_id::TREE_CANOPY_TR,
                        (0, 1) => tile_id::TREE_BASE_BL,
                        _ => tile_id::TREE_BASE_BR,
                    };
                }
                // Tall boulders on left half of selected 2x2 forest cells (~5%)
                if cell_h >= 56 && cell_h < 69 && sub_x == 0 {
                    return if sub_y == 0 {
                        tile_id::BOULDER_TOP
                    } else {
                        tile_id::BOULDER_BASE
                    };
                }
            }
            Biome::Meadow => {
                // Occasional lone shade tree in meadows (~3% of 2x2 cells)
                if cell_h < 8 {
                    return match (sub_x, sub_y) {
                        (0, 0) => tile_id::TREE_CANOPY_TL,
                        (1, 0) => tile_id::TREE_CANOPY_TR,
                        (0, 1) => tile_id::TREE_BASE_BL,
                        _ => tile_id::TREE_BASE_BR,
                    };
                }
                // Small mossy rocks scattered sparsely in meadows
                if cell_h >= 8 && cell_h < 16 && sub_x == 1 && sub_y == 1 {
                    return tile_id::ROCK_SMALL;
                }
            }
        }
    }

    // Ground detail tiles per biome
    let gh = hash_coords(seed, tx, ty) & 0x1F;
    match biome {
        Biome::Meadow => match gh {
            0 => tile_id::FLOWER_YELLOW,
            1 => tile_id::FLOWER_WHITE,
            2..=5 => tile_id::CLOVER,
            6..=13 => tile_id::MEADOW_B,
            _ => tile_id::MEADOW_A,
        },
        Biome::Forest => match gh {
            0 => tile_id::DIRT_CLEARING,
            1..=3 => tile_id::CLOVER,
            4..=15 => tile_id::FOREST_FLOOR_B,
            _ => tile_id::FOREST_FLOOR_A,
        },
    }
}

/// Returns `true` if `tid` has a solid collision box on its base.
///
/// Note that `TREE_CANOPY_TL`, `TREE_CANOPY_TR`, and `BOULDER_TOP` intentionally
/// return `false` so entities only collide with the **base** of trees and rocks!
#[inline]
#[must_use]
pub const fn tile_has_base_collision(tid: usize) -> bool {
    matches!(
        tid,
        tile_id::TREE_BASE_BL
            | tile_id::TREE_BASE_BR
            | tile_id::ROCK_SMALL
            | tile_id::BOULDER_BASE
            | tile_id::BORDER_WALL
    )
}

/// Returns the world-space base collision rectangle in pixels for tile `(tx, ty)`, if solid.
#[must_use]
pub fn tile_collision_rect(seed: u32, tx: i32, ty: i32) -> Option<Rect<i32>> {
    let tid = tile_index_at(seed, tx, ty);
    let px = tx * TILE_PX;
    let py = ty * TILE_PX;
    match tid {
        tile_id::BORDER_WALL => Some(rect(vec2(px, py), vec2(8, 8))),
        // Left & right halves of a tree's trunk base form a snug 8x6 collision box at the base!
        tile_id::TREE_BASE_BL => Some(rect(vec2(px + 4, py + 2), vec2(4, 5))),
        tile_id::TREE_BASE_BR => Some(rect(vec2(px, py + 2), vec2(4, 5))),
        // Lower base of rocks & boulders so entities can overlap the upper edge naturally.
        tile_id::ROCK_SMALL => Some(rect(vec2(px + 1, py + 3), vec2(6, 4))),
        tile_id::BOULDER_BASE => Some(rect(vec2(px + 1, py + 1), vec2(6, 6))),
        _ => None,
    }
}

/// Returns `true` if axis-aligned pixel rectangle `hitbox` intersects any solid world base box.
#[must_use]
pub fn rect_collides_world(seed: u32, hitbox: Rect<i32>) -> bool {
    let min_tx = hitbox.position.x.div_euclid(TILE_PX);
    let max_tx = (hitbox.position.x + hitbox.size.x - 1).div_euclid(TILE_PX);
    let min_ty = hitbox.position.y.div_euclid(TILE_PX);
    let max_ty = (hitbox.position.y + hitbox.size.y - 1).div_euclid(TILE_PX);

    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            if let Some(solid) = tile_collision_rect(seed, tx, ty)
                && hitbox.touches(solid)
            {
                return true;
            }
        }
    }
    false
}

/// Streaming `1024x1024` tile world wrapper around [`InfiniteScrolledMap`].
pub struct WorldMap {
    /// Deterministic seed driving `tile_index_at`.
    pub seed: u32,
    /// `agb` virtual scrolled background streaming tiles into a `32x32` VRAM screenblock.
    scroller: InfiniteScrolledMap,
}

impl WorldMap {
    /// Allocates the underlying `32x32` 4bpp [`RegularBackground`] and pre-populates initial tiles
    /// around `initial_cam` so the very first rendered frame is completely filled without pop-in.
    #[must_use]
    pub fn new(seed: u32, initial_cam: Vector2D<i32>) -> Self {
        let bg = RegularBackground::new(
            Priority::P2,
            RegularBackgroundSize::Background32x32,
            TileFormat::FourBpp,
        );
        let mut world = Self {
            seed,
            scroller: InfiniteScrolledMap::new(bg),
        };
        // Pre-warm the smeared initial copy (takes ~11 calls of 2 rows each) before gameplay starts.
        for _ in 0..16 {
            if world.update_scroll(initial_cam) == PartialUpdateStatus::Done {
                break;
            }
        }
        world
    }

    /// Streams newly exposed edge tiles as the camera scrolls to `cam_pos` (in pixels).
    pub fn update_scroll(&mut self, cam_pos: Vector2D<i32>) -> PartialUpdateStatus {
        let seed = self.seed;
        self.scroller.set_scroll_pos(cam_pos, |pos| {
            let idx = tile_index_at(seed, pos.x, pos.y);
            let tileset: &'static TileSet = &backgrounds::WORLD_TILES.tiles;
            let setting: TileSetting = backgrounds::WORLD_TILES.tile_settings[idx];
            (tileset, setting)
        })
    }

    /// Submits the scrolled background layer to the current [`GraphicsFrame`].
    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.scroller.show(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn world_border_and_starter_clearing_invariants(_gba: &mut agb::Gba) {
        let seed = 0xC491_2026;
        // Outer border of the 1024x1024 map must always be solid.
        assert_eq!(tile_index_at(seed, 0, 0), tile_id::BORDER_WALL);
        assert_eq!(tile_index_at(seed, 1023, 1023), tile_id::BORDER_WALL);
        assert!(rect_collides_world(seed, rect(vec2(0, 0), vec2(8, 8))));

        // Starter camp clearing around (512, 512) must be completely walkable.
        assert!(!rect_collides_world(
            seed,
            rect(vec2(START_TILE_X * 8, START_TILE_Y * 8), vec2(8, 16))
        ));

        // Tree canopy and boulder peak must be passable; only bases collide.
        assert!(!tile_has_base_collision(tile_id::TREE_CANOPY_TL));
        assert!(!tile_has_base_collision(tile_id::TREE_CANOPY_TR));
        assert!(!tile_has_base_collision(tile_id::BOULDER_TOP));
        assert!(tile_has_base_collision(tile_id::TREE_BASE_BL));
        assert!(tile_has_base_collision(tile_id::TREE_BASE_BR));
        assert!(tile_has_base_collision(tile_id::BOULDER_BASE));
        assert!(tile_has_base_collision(tile_id::ROCK_SMALL));
    }
}
