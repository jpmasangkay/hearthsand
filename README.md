# 🔥 Hearthsand
[![Ask DeepWiki](https://devin.ai/assets/askdeepwiki.png)](https://deepwiki.com/jpmasangkay/hearthsand)

### 2D Top-Down Elemental Cellular-Automata Sandbox, Autonomous Village Colony Sim & Spell-Crafting ARPG — Pure Rust & WebAssembly

**Sculpt. Irrigate. Ignite. Cast. Pure Rust.**

A high-performance, 60 FPS 2D top-down pixel-art settlement simulation fused with a Noita-style falling-sand elemental sandbox and programmable rune-wand spellcaster, built entirely in Rust with `macroquad`. Features strictly conserved integer hydrology and soil-moisture wicking, thermodynamic chain reactions (fire, wind, lava, magma, ice, acid, oil, gunpowder, buoyant steam & smoke), autonomous Utility-AI villagers with 1 Hz flood-fill reachability and budgeted A* pathfinding, nocturnal predators and crop-foraging wildlife, a playable **Mage Mode** with a 3-slot stackable rune wand builder, a zero-asset `4×4` micro-pixel procedural rasterizer with dirty-chunk + `10 Hz` animated-cell updates, and a 3-pass GLSL ES 1.00 dynamic day/night lightmap pipeline that runs natively and in the browser via WebAssembly.

[![Play in Browser](https://img.shields.io/badge/Play_Online-GitHub_Pages-2EA44F?style=for-the-badge&logo=github&logoColor=white)](https://jpmasangkay.github.io/hearthsand/)
[![Rust](https://img.shields.io/badge/Rust-1.87+-DEA584?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/Edition-2021-blue?style=for-the-badge)](https://www.rust-lang.org)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-WASM-654FF0?style=for-the-badge&logo=webassembly&logoColor=white)](https://webassembly.org)
[![macroquad](https://img.shields.io/badge/macroquad-0.4-F7A41D?style=for-the-badge)](https://github.com/not-fl3/macroquad)
[![GLSL ES](https://img.shields.io/badge/Shaders-GLSL_ES_1.00-4E8EE9?style=for-the-badge)](https://www.khronos.org/opengles/)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL--2.0-yellow.svg?style=for-the-badge)](LICENSE)

🕹️ **Play in Browser (GitHub Pages):** [jpmasangkay.github.io/hearthsand](https://jpmasangkay.github.io/hearthsand/)  
🖥️ **Repository:** [github.com/jpmasangkay/hearthsand](https://github.com/jpmasangkay/hearthsand)

---

## 📸 Screenshots

<!-- Replace these placeholder slots with captured PNGs/GIFs -->

| God Cursor & Trench Irrigation | Mage Mode & 3-Rune Wand Builder |
| :---: | :---: |
| `![God Cursor village and irrigation](docs/screenshots/god_cursor.png)` | `![Mage mode casting rune spells](docs/screenshots/mage_wand.png)` |

| Dynamic Night Lighting & Campfires | Elemental Chain Reactions (Lava / Gunpowder / Acid) |
| :---: | :---: |
| `![Nighttime dynamic lightmap and fireflies](docs/screenshots/night_lighting.png)` | `![Elemental chain reactions](docs/screenshots/elemental_reactions.png)` |

---

## ✨ Features

### 🌊 Elemental Cellular Automata & Conserved Hydrology

- **Conserved integer water physics** — every tile tracks an exact integer `water_volume` (`0..=100`) conserved across shallow/deep water leveling, trench canal flow, steam evaporation, rainstorm deposition, soil puddle absorption, and villager water buckets.
- **Soil moisture wicking & irrigation** — water wicks laterally into adjacent dirt and tilled soil within 2 tiles, converting dry `Dirt` and `TilledSoil` into rich `WetSoil` that accelerates 4-stage crop growth by `4×`.
- **Chunk-awake simulation grid** — the `160×100` tile world is partitioned into `10×7` (`16×16`) spatial chunks with sleep/wake bitmasks so quiescent regions skip redundant CA work.
- **3 deterministic procedural presets** — domain-warped Fbm Perlin terrain generators for **Lush River Island** (meandering S-curve river + 3–5% rocky outcrops), **Volcanic Crater Village** (warped lava/magma caldera, basalt outcrops, ash drifts, and mountain springs), and **Drought Valley** (winding canyon stream and noisy coastal cliffs).

### 🔥 Thermodynamics, Chain Reactions & Weather

- **Per-tile heat conduction & ignition** — continuous temperature field diffuses heat across neighbors and ignites flammable materials (`Grass`, `Tree`, `BerryBush`, `Crop`, `WoodWall`, `Cabin`, `Oil`, `Gunpowder`, `Campfire`) above their ignition thresholds.
- **Multi-element interactions**:
  - **Lava & Water / Ice** → quenches into `Stone` or `Magma`, boils water into rising `Steam` clouds, and melts `Ice` into `ShallowWater`.
  - **Gunpowder & Oil** → high-velocity chain-reaction explosions (capped at 8 detonations/tick to prevent frame spikes) that carve craters, launch ballistic debris particles, and trigger decaying camera trauma shake.
  - **Acid** → dissolves `Stone`, `WoodWall`, and organic matter while emitting toxic green `AcidGas` plumes.
- **Buoyant gas & condensation cycle** — `Steam` rises and drifts with the wind vector, cools aloft, and condenses back into conserved water on nearby water bodies or soil without overwriting farmland.
- **Dynamic weather & wind** — live drifting wind compass plus `Clear`, `Rainstorm` (hydrates crops and quenches wildfires), and `Heatwave` (accelerates soil drying and wildfire spread).

### 🧑‍🌾 Autonomous Utility-AI Villagers & Budgeted Pathfinding

- **Scored Utility AI with 20% hysteresis** — villagers evaluate urges every `0.5s` across `FleeHazard`, `Extinguish`, `Eat`, `Warm`, `HarvestHaul`, `FarmIrrigate`, `LumberBuild`, and `Idle`, with a `1.20×` commitment bonus to prevent task thrashing and an immediate 3-tile hazard flee override.
- **1 Hz flood-fill reachability & 30s failure blacklist** — connected-component reachability is flood-filled from villagers at most once per simulated second so `VillageScan` drops walled-off or unreachable crops, trees, bushes, and fires; any A* no-path result blacklists `(job, target)` for `30` sim-seconds.
- **0-1 BFS autonomous trench engineering** — villagers automatically plan and dig irrigation trenches from rivers and mountain springs to dry farmland, prioritizing existing canals (cost `0`) over raw terrain (cost `1`).
- **Round-robin budgeted A* pathfinding** — strictly capped at `MAX_ASTAR_PER_FRAME = 8` searches per tick with automatic path invalidation on `world.last_terrain_change_tick`.
- **Day/night wildlife ecology** — daytime rabbits hop toward crops and forage across fields; nocturnal wolves prowl from the wilds at night, flee lit campfire light radii, and can be stunned by lightning spells.

### 🧙‍♂️ Mage Mode & 3-Rune Programmable Wand Builder

- **Direct avatar control (`Tab`)** — drop into the world as a robe-and-hat Mage with `WASD` movement, mouse-aim reticle, and a `Space` dash with sub-stepped wall collision and translucent afterimage ghosts.
- **6 elemental spell payloads** — `Fireball`, `Hydro Canal Jet`, `Chain Lightning`, `Gravity Vortex`, `Overgrowth Seed Bomb`, and `Terra Wall`.
- **5 stackable modifier runes (3 wand slots)** — `Multi-Cast` (`×3` spread), `Homing`, `Orbit`, `Explosive Impact`, and `Electric Charge`. Every `6 × 5` payload-rune combination stacks deterministically (30+ distinct spell behaviors).

### 🎨 Unified 4×4 Micro-Pixel Engine & 3-Pass Dynamic Lighting

- **Zero external image assets** — no PNG files or sprite atlases are loaded. The entire `160×100` world is rasterized at `4×4` micro-pixels per cell (`640×400`) from a flat 20-material×4-shade palette table with organic soft-ground dithering, exposed cliff/bank rim darkening, water top-row highlights, and minimum night/rain luminance floors.
- **Incremental `TerrainRasterizer`** — tracks 64-bit FNV-1a chunk signatures to re-rasterize only dirty chunks (+ cardinal neighbors) and refreshes pre-indexed animated cells (`Water`, `Lava`, `Fire`, `Acid`) on a `10 Hz` clock (`0.153 ms/frame` in `--release`, `13.7×` faster than full-frame rasterization).
- **Unified micro-pixel figures & 3×3 thought icons** — villagers, critters, Mage, projectiles, drops, 1-row ground shadows, and `3×3` colored overhead thought icons are drawn as rectangles on the exact same micro-pixel grid and Y-sorted with trees and cabins.
- **3-Pass GLSL ES 1.00 dynamic lightmap** — renders the scene to a nearest-filtered render target, builds a `1/4`-resolution linear-filtered lightmap (`640×400` at `2×` DPI) with time-of-day ambient curves (`Dawn`, `Day`, `Dusk`, `Night`) and the nearest `64` radial glow lights (campfires, lava, fire, cabin windows, fireflies, spells), and composites via hardware multiply + additive bloom and night rim lighting.

### 🖥️ Clamped Dark-Pixel HUD & High-DPI Layout Engine

- **Catppuccin Mocha-inspired dark pixel UI** — sharp-cornered panels (`#1e1e2e` base, `#313244` surface, `#cdd6f4` text, `#89b4fa` accent) on a strict `8 px` spacing grid with two font sizes (`14 px` / `18 px`) and a fixed per-font cap-height baseline so all button and tab labels align identically.
- **7 bottom category tabs & WCAG auto-contrast swatches** — `Terrain`, `Liquids`, `Fire & Heat`, `Life`, `Build`, `Spells`, and `Tools` with color-filled element buttons that automatically select dark or light text for contrast, plus a clickable `Brush N [-] [+]` control.
- **Window-clamped panels & edge-flipping cursor tooltip** — every panel is clamped inside window bounds (verified headlessly at `960×600` and `1280×800`), and the compact cursor inspector flips left/up near screen edges.
- **`0.25x` snapped zoom** — mouse wheel zoom snaps to multiples of `0.25x` by default so micro-pixels map to even integer logical pixels on high-DPI displays (toggleable in `Menu`).

---

## 🛠️ Tech Stack & Architecture Modules

| Module / Component | Purpose |
|--------------------|---------|
| **Rust 2021 Edition (MSRV 1.87)** | Systems programming language providing deterministic performance and memory safety without `unsafe` |
| **`src/main.rs`** | Fixed `60 Hz` simulation accumulator (max 5 steps/frame), camera/input handling, 3-pass render loop, and `--release` night benchmark harness |
| **`src/world/mod.rs` & `types.rs`** | `160×100` `World` grid, `16×16` chunk wake bitmasks, 23 `Material` variants, `Cell` bitflags, gas & debris particles |
| **`src/world/gen.rs`** | Deterministic Fbm Perlin & domain-warped terrain generators for all 3 presets and starter settlement stamping |
| **`src/world/water.rs` & `ca.rs`** | Conserved integer water leveling, soil moisture wicking, heat conduction, fire/wind spread, lava/ice/acid/gunpowder CA, crop growth |
| **`src/agents/utility_ai.rs` & `villager.rs`** | Reachability-filtered `VillageScan`, scored Utility AI with 20% hysteresis, trench digging, farming, chopping, building, firefighting |
| **`src/agents/pathfinding.rs`** | 1 Hz cached flood-fill connected components, 0-1 BFS canal planner, round-robin budgeted A* (max 8/tick), 30s target failure blacklist |
| **`src/agents/mage.rs` & `spells.rs`** | Playable Mage movement, sub-stepped dash collision, 6 payloads × 5 stackable runes, homing/orbit/vortex/chain-lightning solvers |
| **`src/agents/figures.rs` & `render.rs`** | Procedural micro-pixel figures, tall props, 1-row shadows, `3×3` thought icons, and Y-sorted draw queue |
| **`src/render/terrain.rs`** | Pure `render_terrain(&World, time) -> Image` (`640×400`) + incremental `TerrainRasterizer` (dirty chunks + `10 Hz` animated cells) |
| **`src/render/lighting.rs` & `world_draw.rs`** | 3-pass GLSL ES 1.00 lighting pipeline (`1/4`-res linear lightmap, nearest-64 glow cap), camera clamp, `#1e1e2e` letterbox, `0.25x` zoom snap |
| **`src/ui/` (`theme.rs`, `layout.rs`, `hud.rs`)** | Fixed-baseline typography, WCAG contrast calculator, pure headless-testable panel layout geometry, God/Mage HUDs, Wand drawer |
| **`src/soak_tests.rs`** | Headless 20-minute (`72,000`-tick) per-preset soak tests, weather/fire/gunpowder chaos soak tests, and 90-run 30-seed soak suite |
| **`macroquad 0.4`** | Cross-platform desktop OpenGL & `wasm32-unknown-unknown` WebGL windowing, rendering, and input |
| **`noise 0.9` & `rand 0.8`** | Deterministic multi-octave Fbm Perlin terrain generation and seeded `StdRng` simulation randomness |

---

## 🚀 Getting Started

### Prerequisites

- [Rust SDK](https://rustup.rs/) (**1.87** or later with `cargo`)
- **macOS**, **Windows**, or **Linux** (on Ubuntu/Debian, install native window/audio headers):
  ```bash
  sudo apt-get install pkg-config libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
  ```

### Installation & Build

```bash
# Clone the repository
git clone https://github.com/jpmasangkay/hearthsand.git
cd hearthsand

# Build and run the native release binary
cargo run --release
```

### Running Formatting, Linting & Automated Tests

```bash
# Check formatting and strict Clippy lints (native + WebAssembly targets)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo clippy --target wasm32-unknown-unknown -- -D warnings

# Run all 47 default unit, layout, rasterizer benchmark, and 20-minute soak/chaos tests
cargo test -- --nocapture

# Run the full 90-run (30 seeds x 3 presets) 20-minute soak & productivity suite
cargo test --release soak_20min_seeds_1_to_30_all_presets -- --ignored --nocapture

# Run the live --release night fit-zoom per-pass rendering benchmark
HEARTHSAND_BENCH_NIGHT=1 cargo run --release
```

### WebAssembly (WASM) Web Build & Local Server

Play online immediately at **[jpmasangkay.github.io/hearthsand](https://jpmasangkay.github.io/hearthsand/)**, or build and serve the static WebGL bundle locally:

```bash
# Add the WebAssembly target and compile the zero-asset .wasm binary
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release

# Stage index.html, hearthsand.wasm, and lockfile-matched mq_js_bundle.js into dist/
mkdir -p dist
cp web/index.html dist/
cp target/wasm32-unknown-unknown/release/hearthsand.wasm dist/
MQ_JS=$(python3 -c '
import json, pathlib, subprocess
meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1"]))
pkg = next(p for p in meta["packages"] if p["name"] == "macroquad")
print(pathlib.Path(pkg["manifest_path"]).parent / "js" / "mq_js_bundle.js")
')
cp "$MQ_JS" dist/mq_js_bundle.js

# Serve over HTTP and open http://localhost:8080
python3 -m http.server --directory dist 8080
```

---

## 🎮 Controls

### Global Hotkeys & Shortcuts

| Action | Shortcut |
|--------|----------|
| **Toggle God Cursor / Mage Mode** | `Tab` |
| **Open / Close Wand Builder Drawer** | `B` |
| **Center Camera on Village** | `Home` |
| **Zoom Camera to Fit Entire World** | `F` |
| **Zoom Camera (`0.25x` Snapped Steps)** | `Mouse Wheel` (toggle **Snap Zoom** in `Menu`) |
| **Simulation Speed: Pause / 1x / 3x / 5x** | `1` / `2` / `3` / `4` (or top-left `Play/Pause` + `1x/3x/5x`) |
| **Cycle World Preset** | `P` (`Lush River Island` → `Volcanic Crater Village` → `Drought Valley`) |
| **Reset World (Next Seed `seed + 1`)** | `R` |
| **Cycle Weather** | `V` (`Clear` / `Rainstorm` / `Heatwave`) |
| **Advance Time-of-Day Phase** | `T` (`Night` / `Dawn` / `Day` / `Dusk`) |
| **Toggle Full Stats Panel** | `F3` |
| **Toggle Controls Overlay** | `?` (`Shift + /`), `F1`, or bottom-right `?` button |
| **Close Active Overlay / Drawer** | `Esc` |

### God Cursor Mode

| Action | Input |
|--------|-------|
| **Pan Camera** | `WASD` / `Arrow Keys` or `Right-Click Drag` |
| **Paint Element / Spawn Entity / Throw Bomb** | `Left-Click` (hold to paint materials/water; click to spawn/throw) |
| **Decrease / Increase Brush Radius (`1`–`8`)** | `[` / `]`, `Shift + Mouse Wheel`, or click `[-]` / `[+]` on HUD |
| **Switch Material & Tool Categories** | Bottom tabs (`Terrain`, `Liquids`, `Fire & Heat`, `Life`, `Build`, `Spells`, `Tools`) |
| **Pause / Resume Simulation** | `Space` |

### Mage Mode & Wand Builder

| Action | Input |
|--------|-------|
| **Move Mage** | `WASD` / `Arrow Keys` (camera follows smoothly) |
| **Aim & Cast Active Wand Spell** | `Mouse Aim` + `Left-Click` |
| **Arcane Dash (with Afterimage Trail)** | `Space` |
| **Select Payload / Slot Modifier Runes** | Click payload or rune in **Wand Builder** (`B`); click slot `1`–`3` on bottom HUD |
| **Clear Rune Slot** | `Right-Click` rune slot (or click `Clear` in **Wand Builder**) |

---

## 👥 Author

Built with ❤️ by **John Paul Masangkay** ([@jpmasangkay](https://github.com/jpmasangkay))

---

## 👏 Acknowledgements

- **Fedor Logachev (`not-fl3`) & the `macroquad` contributors** — Ergonomic, zero-boilerplate Rust game and WebGL rendering framework
- **Nolla Games (*Noita*) & *Dwarf Fortress*** — Inspiration for emergent cellular-automata alchemy, programmable wand crafting, and autonomous colony storytelling
- **The Rust Community** — For `noise`, `rand`, and the `wasm32-unknown-unknown` toolchain ecosystem

---

## 📄 License

This project is licensed under the Mozilla Public License 2.0 ([MPL-2.0](LICENSE) — see the [official text](https://www.mozilla.org/en-US/MPL/2.0/)).
