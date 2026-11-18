# Coffee Company

Welcome to the Capi Hunt!


## Table of Contents

- [What is the Capi Hunt?](#what-is-the-capi-hunt?)
- [How do I play?](#how-do-i-play)
- [Licenses and Acknowledgments](#licenses-and-acknowledgments)


### What is the Capi Hunt?

**CapiHunt** is a Game Boy Advance wilderness survival game built in `#![no_std]` Rust with [`agb`](https://github.com/agbrs/agb) and `agb_tracker`. Explore a `1024x1024` tile world of meadows and forests, hunt peaceful Capybaras ("Capis"), light scattered campfires, roast meat (~10 seconds per piece), and eat before your hunger runs out!


### How do I play?

#### Controls (Game Boy Advance)

| Input | Action |
| --- | --- |
| **D-Pad** | Move hunter / navigate menus / aim bow |
| **Hold L** | Run (faster movement, startles Capis from farther away) |
| **Select** | Switch active weapon (**Blade** <-> **Bow**) |
| **Button A** | Swing **Blade** (when Blade selected) |
| **Hold A** | Draw & loose **Bow** arrow (when Bow selected; uses limited arrows found in the world) |
| **Button R** | Contextual survival action: **Light Fire** / **Cook Raw Meat** / **Grab Cooked Meat** / **Eat Cooked Meat** |
| **Start** | Pause menu (`Resume`, `Save` across 3 SRAM slots, `Options` [Music/Sound], `Exit`) |
| **Button B** | Back / cancel in menus / exit Credits |

#### Building & Running

```sh
# Check & lint
cargo check
cargo clippy

# Build debug & release GBA binaries
cargo build
cargo build --release

# Run in mGBA (requires mgba-qt on PATH)
cargo run --release

# Produce a header-fixed .gba ROM for real GBA hardware / flashcarts
agb-gbafix target/thumbv4t-none-eabi/release/CapiHunt -o CapiHunt.gba
```


### Licenses and Acknowledgments

- Game source code and original assets are licensed under MIT.
- Built using the [`agb`](https://github.com/agbrs/agb) library, licensed under the Mozilla Public License 2.0.