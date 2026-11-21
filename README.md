# w3i-rs

Reader and writer for Warcraft III map info files, `war3map.w3i`: what
World Editor's Scenario menu sets. The map's name, author and
description, camera bounds and size, flags, loading and prologue
screens, fog, weather, light and water, players and forces, tech tree
changes and random tables. A library, a small CLI, and a text form for
people; the [wc3-rs](../wc3-rs) editor opens `.w3i` files as that text.

Reading and writing are exact: `to_bytes` gives back the bytes `parse`
read, and so does the text form, on every map of the game, a corpus of
custom maps and the maps in `maps/` (210 files).

## Format

Little-endian. Strings end with a NUL. A field marked with a version is
there from that version on; 18 is Reign of Chaos, 25 The Frozen Throne,
28 1.31, 31 1.32 and later. No other versions are read.

| type | field |
|---|---|
| u32 | format version: 18, 25, 28, 31 |
| u32 | saves: how many times the map was saved |
| u32 | editor version |
| u32 × 4 | (28) the game version that saved it: major, minor, patch, build |
| string × 4 | name, author, description, players recommended |
| f32 × 8 | camera bounds: x, y of bottom left, top right, top left, bottom right |
| i32 × 4 | camera bounds complements: left, right, bottom, top |
| i32 × 2 | playable width, height |
| u32 | flags (below) |
| u8 | tileset letter (`L` Lordaeron Summer ...) |
| i32 | loading screen number, -1 none |
| string | (25) custom loading screen model |
| string × 3 | loading screen text, title, subtitle |
| i32 | (25) game data set; (18) prologue screen number |
| string | (25) custom prologue screen path |
| string × 3 | prologue text, title, subtitle |
| i32, f32 × 3, u8 × 4 | (25) fog: style, start z, end z, density, color RGBA |
| rawcode | (25) global weather, 0 none |
| string | (25) sound environment |
| u8 | (25) custom light tileset letter, 0 the map's own |
| u8 × 4 | (25) water tint RGBA |
| u32 | (28) script language: 0 JASS, 1 Lua |
| u32 × 2 | (31) supported modes (1 SD, 2 HD, 3 both), game data version |
| u32 + players | number, type, race, fixed start (i32 each), name, start x, y (f32), ally low and high priorities (u32 masks); (31) enemy low and high priorities |
| u32 + forces | flags, players (u32 mask), name |
| u32 + upgrades | players, id, level (from 0), availability (0 unavailable, 1 available, 2 researched) |
| u32 + techs | players, id |
| u32 + random unit tables | number, name, u32 + column types (0 unit, 1 building, 2 item), u32 + rows: chance, an id per column |
| u32 + random item tables | (25) number, name, u32 + sets: u32 + items: chance, id |

Player types: 1 human, 2 computer, 3 neutral, 4 rescuable. Races: 0
selectable, 1 human, 2 orc, 3 undead, 4 night elf. Player masks have a
bit per player, bit 0 for Player 1.

Map flags: 0x1 hide minimap in preview, 0x2 modify ally priorities, 0x4
melee map, 0x8 playable size was large and never reduced, 0x10 masked
areas partially visible, 0x20 fixed player settings for custom forces,
0x40 custom forces, 0x80 custom tech tree, 0x100 custom abilities, 0x200
custom upgrades, 0x400 properties menu opened, 0x800 waves on cliff
shores, 0x1000 waves on rolling shores, 0x2000 terrain fog, 0x4000
requires The Frozen Throne, 0x8000 item classification, 0x10000 water
tinting, 0x20000 accurate probability, 0x40000 custom ability skins.
Other bits are kept as they are.

Force flags: 0x1 allied, 0x2 allied victory, 0x8 shared vision, 0x10
shared unit control, 0x20 shared advanced unit control. (Some
descriptions give shared vision as 0x4; no map in the corpus has 0x4,
and 0x8 is common.)

**Cut files.** Map optimizers drop the lists after the forces (or after
any later list) and can leave a stray byte or three; the game reads such
a map. Where a list's count would start, fewer than 4 bytes left mean the
file stops there: `Info::cut` says after which list, and keeps those
bytes, so the file writes back as it was.

Versions 28 and 31 follow the format as other tools (HiveWE) read it;
every map at hand is 18 or 25, so those two are checked by tests only.

## Library

```rust
let mut info = w3i_rs::parse(&std::fs::read("war3map.w3i")?)?;
info.author = "Me".into();
info.players[0].race = 2; // orc
std::fs::write("war3map.w3i", info.to_bytes()?)?;
```

`Info::new(version)` is an empty file. Fields a version doesn't have stay
at their defaults and aren't written. Errors are `parser_rs::BinError`,
with the byte offset; counts are checked against what's left before
anything is allocated.

## Text

`text::to_text` and `text::from_text`: BNI, a `[w3i]` section with the
map's fields, then a section per player, force, upgrade, tech, unit table
and item table, in file order.

```ini
[w3i]
version=25
saves=17076
name="TRIGSTR_001"
camera_bounds=-5248,-10368,4992,4224,-5248,4224,4992,-10368
flags=0x0001DC68
tileset=C
fog_color=0,0,0,255
weather=0x00000000

[player]
number=0
type=1
race=1
name="TRIGSTR_005"
ally_high=0x000001FE

[force]
flags=0x0000000B
players=0x000001FF
name="TRIGSTR_004"

[unit_table]
number=0
name="Group"
columns=0,2
row=50,hfoo,YYI1

[item_table]
number=3
name="Drops"
set=10,ratf,90,rde1
```

- What a value means isn't in the text: an editor shows it, as the
  wc3-rs editor does with inlay hints. The crate has the tables of what
  the format's values mean (`FLAGS`, `FORCE_FLAGS`, `PLAYER_TYPES`,
  `RACES`, `FOG_STYLES`, `AVAILABILITY`, `COLUMNS`, `SCRIPTS`, `MODES`),
  each value with an English name and the key of World Editor's string
  for it (`WESTRING_RACE_HUMAN`), so an editor shows the game's words, in
  the game's language. The tilesets' names are the game's files' alone.
- Strings are quoted. A line break or `"` goes between quoted parts as an
  escape, `"Line one"\n"Line two"` (`bni_rs::string`).
- Masks and flags are hex; numbers are the shortest decimal that reads
  back as the same `f32` (a NaN as `0x` and its bits); ids are their four
  characters, or `0x` and hex.
- `row=chance,id,...` has an id per column; `set=chance,id,chance,id,...`
  is one item set, `set=` an empty one.
- A cut file says so in `[w3i]`: `cut=forces` and `cut_bytes=255`.
- The version decides which fields there are: one it doesn't have is an
  error. A field left out takes its default.

## CLI

```sh
w3i-rs war3map.w3i                       # read and write back: a check
w3i-rs --text war3map.w3i                # the text form
w3i-rs --from-text map.ini war3map.w3i   # text back to a file
```

## Tests

```sh
cargo test
W3I_CORPUS=<folder of map folders> cargo test --release corpus -- --ignored
```

The corpus test reads every `<map>/war3map.w3i`, writes it back, and goes
through the text form too; both must give the file's bytes.
