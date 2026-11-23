//! Reader and writer for Warcraft III map info files, `war3map.w3i`: what
//! World Editor's Scenario menu sets. The map's name and description, its
//! camera bounds and size, the loading screen, fog and weather, players
//! and forces, tech tree changes and random tables.
//!
//! ```no_run
//! let bytes = std::fs::read("war3map.w3i").unwrap();
//! let mut info = w3i_rs::parse(&bytes).unwrap();
//! info.author = "Me".into();
//! std::fs::write("war3map.w3i", info.to_bytes().unwrap()).unwrap();
//! ```
//!
//! Four versions are known: 18 (Reign of Chaos), 25 (The Frozen Throne),
//! 28 (1.31) and 31 (1.32 and later). What each has is in the README.
//! Reading and writing are exact: `to_bytes` gives back the bytes `parse`
//! read. For people, a file has a text form ([`text`]).

pub mod text;

pub use parser_rs::bin::Rawcode;
use parser_rs::bin::{Reader, Writer};
pub use parser_rs::{BinError, BinResult};

/// The versions this crate reads and writes: Reign of Chaos, The Frozen
/// Throne, 1.31, 1.32.
pub const VERSIONS: [u32; 4] = [18, 25, 28, 31];

/// The game versions a format version is for, in words.
pub fn describe(version: u32) -> &'static str {
    match version {
        18 => "Reign of Chaos",
        25 => "The Frozen Throne",
        28 => "1.31",
        31 => "1.32 and later",
        _ => "an unknown game",
    }
}

/// What a version has, past Reign of Chaos's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Has {
    /// The Frozen Throne (25): a loading screen model, game data set,
    /// prologue path, fog, weather, sound environment, light and water,
    /// random item tables.
    pub tft: bool,
    /// 1.31 (28): the game version that saved it, the script language.
    pub v28: bool,
    /// 1.32 (31): supported modes, game data version, enemy priorities.
    pub v31: bool,
}

impl Has {
    pub fn of(version: u32) -> Has {
        Has {
            tft: version >= 25,
            v28: version >= 28,
            v31: version >= 31,
        }
    }
}

/// The fog World Editor's Map Options set (The Frozen Throne).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fog {
    /// 0 none, 1 linear, 2 exponential, 3 exponential squared.
    pub style: i32,
    pub start_z: f32,
    pub end_z: f32,
    pub density: f32,
    /// As stored: blue, green, red, alpha (World Editor's script reads
    /// it so: Blizzard's SavageStorm).
    pub color: [u8; 4],
}

/// One player slot.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Player {
    /// 0 for Player 1.
    pub number: i32,
    /// 1 human, 2 computer, 3 neutral, 4 rescuable.
    pub kind: i32,
    /// 1 human, 2 orc, 3 undead, 4 night elf; 0 selectable.
    pub race: i32,
    /// 1: a fixed start location.
    pub fixed_start: i32,
    pub name: String,
    pub start_x: f32,
    pub start_y: f32,
    /// Players this one's ally priority is low or high for, a bit each.
    pub ally_low: u32,
    pub ally_high: u32,
    /// Since 1.32.
    pub enemy_low: u32,
    pub enemy_high: u32,
}

/// One force (team).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Force {
    /// See [`FORCE_FLAGS`].
    pub flags: u32,
    /// Its players, a bit each.
    pub players: u32,
    pub name: String,
}

/// An upgrade's availability for some players.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Upgrade {
    /// The players, a bit each.
    pub players: u32,
    pub id: Rawcode,
    /// Counting from 0.
    pub level: i32,
    /// 0 unavailable, 1 available, 2 researched.
    pub availability: i32,
}

/// A unit, building, ability or item made unavailable for some players.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tech {
    /// The players, a bit each.
    pub players: u32,
    pub id: Rawcode,
}

/// A random unit table (`Advanced > Random Groups`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnitTable {
    pub number: i32,
    pub name: String,
    /// What each column is: 0 a unit, 1 a building, 2 an item.
    pub columns: Vec<i32>,
    /// A row per group: its chance in percent, then an id per column.
    pub rows: Vec<(i32, Vec<Rawcode>)>,
}

/// A random item table (`Advanced > Random Item Tables`, The Frozen
/// Throne).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemTable {
    pub number: i32,
    pub name: String,
    /// Item sets: each a list of items with their chance in percent.
    pub sets: Vec<Vec<(i32, Rawcode)>>,
}

/// The lists after the forces, which a file can stop before: map
/// optimizers cut them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    Forces,
    Upgrades,
    Techs,
    UnitTables,
}

impl Part {
    /// Its name in the text form, as in the README.
    pub fn name(self) -> &'static str {
        match self {
            Part::Forces => "forces",
            Part::Upgrades => "upgrades",
            Part::Techs => "techs",
            Part::UnitTables => "unit_tables",
        }
    }
    pub fn from_name(name: &str) -> Option<Part> {
        [Part::Forces, Part::Upgrades, Part::Techs, Part::UnitTables]
            .into_iter()
            .find(|p| p.name() == name)
    }
}

/// Where a cut file stops, and the bytes after that: fewer than a count
/// takes, which the game doesn't read.
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    /// The last list the file has.
    pub after: Part,
    /// At most 3.
    pub bytes: Vec<u8>,
}

/// A whole `war3map.w3i`. Fields a version doesn't have are kept at
/// their defaults and not written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Info {
    /// One of [`VERSIONS`].
    pub version: u32,
    /// How many times the map was saved.
    pub saves: u32,
    /// The World Editor that saved it.
    pub editor_version: u32,
    /// Since 1.31: the game that saved it, major, minor, patch, build.
    pub game_version: [u32; 4],
    pub name: String,
    pub author: String,
    pub description: String,
    pub players_recommended: String,
    /// The camera bounds' four corners, x and y each: bottom left, top
    /// right, top left, bottom right.
    pub camera_bounds: [f32; 8],
    /// The cells the camera bounds leave out: left, right, bottom, top.
    pub camera_complements: [i32; 4],
    pub playable_width: i32,
    pub playable_height: i32,
    /// See [`FLAGS`].
    pub flags: u32,
    /// The terrain's base tileset, its letter.
    pub tileset: u8,
    /// Which of the campaigns' loading screens, or -1 for none (or a
    /// custom one).
    pub loading_screen: i32,
    /// The Frozen Throne: a custom loading screen's model.
    pub loading_screen_model: String,
    pub loading_text: String,
    pub loading_title: String,
    pub loading_subtitle: String,
    /// The game data set (0 the standard one; maps also store -1).
    pub game_data_set: i32,
    /// The Frozen Throne: a custom prologue screen's path.
    pub prologue_path: String,
    pub prologue_text: String,
    pub prologue_title: String,
    pub prologue_subtitle: String,
    /// The Frozen Throne.
    pub fog: Fog,
    /// The Frozen Throne: the global weather effect, none when all zero.
    pub weather: Rawcode,
    /// The Frozen Throne.
    pub sound_environment: String,
    /// The Frozen Throne: the tileset whose lighting the map uses, 0 for
    /// its own.
    pub light_tileset: u8,
    /// The Frozen Throne: the water's tint, red, green, blue, alpha.
    pub water_color: [u8; 4],
    /// Since 1.31: 0 JASS, 1 Lua.
    pub script: u32,
    /// Since 1.32: 1 SD, 2 HD, 3 both.
    pub supported_modes: u32,
    /// Since 1.32.
    pub game_data_version: u32,
    pub players: Vec<Player>,
    pub forces: Vec<Force>,
    pub upgrades: Vec<Upgrade>,
    pub techs: Vec<Tech>,
    pub unit_tables: Vec<UnitTable>,
    /// The Frozen Throne.
    pub item_tables: Vec<ItemTable>,
    /// Set when the file stops early (map optimizers cut it): the lists
    /// after [`Cut::after`] are then empty and not written.
    pub cut: Option<Cut>,
}

/// A value's meaning: the value, its name in English, and the key of
/// World Editor's string for it (`UI\WorldEditStrings.txt`), empty when
/// the editor has none. An editor shows the game's string, in the game's
/// language, and the English name without the game's files.
pub type Meaning<T> = (T, &'static str, &'static str);

/// The flags World Editor sets, by bit; the rest are kept as they are.
pub const FLAGS: [Meaning<u32>; 19] = [
    (0x0000_0001, "hide minimap in preview screens", "WESTRING_MAPFLAG_HIDEMINIMAP"),
    (0x0000_0002, "modify ally priorities", "WESTRING_ALLYPRI_CUSTOM"),
    (0x0000_0004, "melee map", "WESTRING_STAT_MELEE"),
    (0x0000_0008, "use custom tileset", "WESTRING_MODTEX_USECUSTOM"),
    (0x0000_0010, "masked areas are partially visible", "WESTRING_MAPFLAG_USEDARKMASK"),
    (0x0000_0020, "fixed player settings for custom forces", "WESTRING_USECUSTOMPLAYERS"),
    (0x0000_0040, "use custom forces", "WESTRING_USECUSTOMFORCES"),
    (0x0000_0080, "use custom tech tree", "WESTRING_USECUSTOMTECHTREE"),
    (0x0000_0100, "use custom abilities", "WESTRING_USECUSTOMABILITIES"),
    (0x0000_0200, "use custom upgrades", "WESTRING_USECUSTOMUPGRADES"),
    (0x0000_0400, "map properties menu opened at least once", ""),
    (0x0000_0800, "show water waves on cliff shores", "WESTRING_MAPFLAG_CLIFFWAVES"),
    (0x0000_1000, "show water waves on rolling shores", "WESTRING_MAPFLAG_ROLLINGWAVES"),
    (0x0000_2000, "use terrain fog", "WESTRING_MAPOPTIONS_USEFOG"),
    (0x0000_4000, "requires The Frozen Throne", "WESTRING_MAPDESC_EXPANSION"),
    (0x0000_8000, "use item classification system", "WESTRING_MAPFLAG_USEITEMCLASS"),
    (0x0001_0000, "use water tinting color", "WESTRING_MAPOPTIONS_WATERCOLOR"),
    (0x0002_0000, "use accurate probability for calculations", ""),
    (0x0004_0000, "use custom ability skins", ""),
];

/// A force's flags, by bit.
pub const FORCE_FLAGS: [Meaning<u32>; 5] = [
    (0x01, "allied", "WESTRING_ALLIED"),
    (0x02, "allied victory", "WESTRING_ALLIEDVICTORY"),
    (0x08, "shared vision", "WESTRING_SHAREVISION"),
    (0x10, "shared unit control", "WESTRING_SHARECONTROL"),
    (0x20, "shared advanced unit control", "WESTRING_SHARECONTROLADV"),
];

/// [`Player::kind`].
pub const PLAYER_TYPES: [Meaning<i32>; 4] = [
    (1, "user", "WESTRING_PLAYER_HUMAN"),
    (2, "computer", "WESTRING_PLAYER_COMPUTER"),
    (3, "neutral", "WESTRING_PLAYER_NEUTRAL"),
    (4, "rescuable", "WESTRING_PLAYER_RESCUE"),
];

/// [`Player::race`].
pub const RACES: [Meaning<i32>; 5] = [
    (0, "selectable", "WESTRING_RACE_SELECTABLE"),
    (1, "human", "WESTRING_RACE_HUMAN"),
    (2, "orc", "WESTRING_RACE_ORC"),
    (3, "undead", "WESTRING_RACE_UNDEAD"),
    (4, "night elf", "WESTRING_RACE_NIGHTELF"),
];

/// [`Fog::style`].
pub const FOG_STYLES: [Meaning<i32>; 4] = [
    (0, "none", "WESTRING_NONE"),
    (1, "linear", "WESTRING_FOGSTYLE_LINEAR"),
    (2, "exponential", "WESTRING_FOGSTYLE_EXP1"),
    (3, "exponential squared", "WESTRING_FOGSTYLE_EXP2"),
];

/// [`Upgrade::availability`].
pub const AVAILABILITY: [Meaning<i32>; 3] = [
    (0, "unavailable", "WESTRING_UPGRADE_UNAVAILABLE"),
    (1, "available", "WESTRING_UPGRADE_AVAILABLE"),
    (2, "researched", "WESTRING_UPGRADE_RESEARCHED"),
];

/// [`UnitTable::columns`].
pub const COLUMNS: [Meaning<i32>; 3] = [
    (0, "unit", "WESTRING_TECHTYPE_NORMAL"),
    (1, "building", "WESTRING_TECHTYPE_BUILDING"),
    (2, "item", "WESTRING_TECHTYPE_ITEM"),
];

/// [`Info::script`].
pub const SCRIPTS: [Meaning<u32>; 2] = [(0, "JASS", ""), (1, "Lua", "")];

/// [`Info::supported_modes`].
pub const MODES: [Meaning<u32>; 3] = [(1, "SD", ""), (2, "HD", ""), (3, "SD and HD", "")];

/// The meaning of `value` in `table`.
pub fn meaning<T: PartialEq + Copy>(table: &[Meaning<T>], value: T) -> Option<Meaning<T>> {
    table.iter().find(|(v, _, _)| *v == value).copied()
}

/// The meanings of the bits set in `flags`, in bit order, and the bits
/// none names.
pub fn bits(table: &[Meaning<u32>], flags: u32) -> (Vec<Meaning<u32>>, u32) {
    let set = table.iter().filter(|(b, _, _)| flags & b != 0).copied().collect();
    let rest = table.iter().fold(flags, |n, (b, _, _)| n & !b);
    (set, rest)
}

/// The English names of the map flags set in `flags`, in bit order.
pub fn flag_names(flags: u32) -> Vec<&'static str> {
    bits(&FLAGS, flags).0.into_iter().map(|(_, name, _)| name).collect()
}

impl Info {
    /// An empty file in `version`: no players, no forces, no loading
    /// screen.
    pub fn new(version: u32) -> Info {
        Info {
            version,
            loading_screen: -1,
            ..Info::default()
        }
    }

    /// What its version has.
    pub fn has(&self) -> Has {
        Has::of(self.version)
    }

    /// The file's bytes.
    pub fn to_bytes(&self) -> BinResult<Vec<u8>> {
        if !VERSIONS.contains(&self.version) {
            return Err(unknown(self.version));
        }
        let has = self.has();
        let mut w = Writer::new();
        w.u32(self.version);
        w.u32(self.saves);
        w.u32(self.editor_version);
        if has.v28 {
            for n in self.game_version {
                w.u32(n);
            }
        }
        for s in [
            &self.name,
            &self.author,
            &self.description,
            &self.players_recommended,
        ] {
            w.cstr(s)?;
        }
        for f in self.camera_bounds {
            w.f32(f);
        }
        for n in self.camera_complements {
            w.i32(n);
        }
        w.i32(self.playable_width);
        w.i32(self.playable_height);
        w.u32(self.flags);
        w.u8(self.tileset);
        w.i32(self.loading_screen);
        if has.tft {
            w.cstr(&self.loading_screen_model)?;
        }
        w.cstr(&self.loading_text)?;
        w.cstr(&self.loading_title)?;
        w.cstr(&self.loading_subtitle)?;
        w.i32(self.game_data_set);
        if has.tft {
            w.cstr(&self.prologue_path)?;
        }
        w.cstr(&self.prologue_text)?;
        w.cstr(&self.prologue_title)?;
        w.cstr(&self.prologue_subtitle)?;
        if has.tft {
            w.i32(self.fog.style);
            w.f32(self.fog.start_z);
            w.f32(self.fog.end_z);
            w.f32(self.fog.density);
            w.bytes(&self.fog.color);
            w.rawcode(self.weather);
            w.cstr(&self.sound_environment)?;
            w.u8(self.light_tileset);
            w.bytes(&self.water_color);
        }
        if has.v28 {
            w.u32(self.script);
        }
        if has.v31 {
            w.u32(self.supported_modes);
            w.u32(self.game_data_version);
        }

        w.count(self.players.len())?;
        for p in &self.players {
            w.i32(p.number);
            w.i32(p.kind);
            w.i32(p.race);
            w.i32(p.fixed_start);
            w.cstr(&p.name)?;
            w.f32(p.start_x);
            w.f32(p.start_y);
            w.u32(p.ally_low);
            w.u32(p.ally_high);
            if has.v31 {
                w.u32(p.enemy_low);
                w.u32(p.enemy_high);
            }
        }
        w.count(self.forces.len())?;
        for f in &self.forces {
            w.u32(f.flags);
            w.u32(f.players);
            w.cstr(&f.name)?;
        }
        let has_part = |part: Part| self.cut.as_ref().is_none_or(|c| c.after >= part);
        if let Some(cut) = &self.cut {
            let kept = |part: Part, empty: bool| empty || has_part(part);
            if !(kept(Part::Upgrades, self.upgrades.is_empty())
                && kept(Part::Techs, self.techs.is_empty())
                && kept(Part::UnitTables, self.unit_tables.is_empty())
                && self.item_tables.is_empty())
                || cut.bytes.len() > 3
            {
                return Err(BinError::new(
                    w.len(),
                    format!(
                        "the file is cut after its {}: the lists after them are empty, and at most 3 bytes follow",
                        cut.after.name()
                    ),
                ));
            }
        }
        if !has_part(Part::Upgrades) {
            return Ok(self.finish_cut(w));
        }
        w.count(self.upgrades.len())?;
        for u in &self.upgrades {
            w.u32(u.players);
            w.rawcode(u.id);
            w.i32(u.level);
            w.i32(u.availability);
        }
        if !has_part(Part::Techs) {
            return Ok(self.finish_cut(w));
        }
        w.count(self.techs.len())?;
        for t in &self.techs {
            w.u32(t.players);
            w.rawcode(t.id);
        }
        if !has_part(Part::UnitTables) {
            return Ok(self.finish_cut(w));
        }
        w.count(self.unit_tables.len())?;
        for t in &self.unit_tables {
            w.i32(t.number);
            w.cstr(&t.name)?;
            w.count(t.columns.len())?;
            for &c in &t.columns {
                w.i32(c);
            }
            w.count(t.rows.len())?;
            for (i, (chance, ids)) in t.rows.iter().enumerate() {
                if ids.len() != t.columns.len() {
                    return Err(BinError::new(
                        w.len(),
                        format!(
                            "random unit table {}: row {i} has {} ids for {} columns",
                            t.number,
                            ids.len(),
                            t.columns.len()
                        ),
                    ));
                }
                w.i32(*chance);
                for &id in ids {
                    w.rawcode(id);
                }
            }
        }
        if self.cut.is_some() {
            return Ok(self.finish_cut(w));
        }
        if has.tft {
            w.count(self.item_tables.len())?;
            for t in &self.item_tables {
                w.i32(t.number);
                w.cstr(&t.name)?;
                w.count(t.sets.len())?;
                for set in &t.sets {
                    w.count(set.len())?;
                    for &(chance, id) in set {
                        w.i32(chance);
                        w.rawcode(id);
                    }
                }
            }
        }
        Ok(w.into_bytes())
    }

    fn finish_cut(&self, mut w: Writer) -> Vec<u8> {
        if let Some(cut) = &self.cut {
            w.bytes(&cut.bytes);
        }
        w.into_bytes()
    }
}

fn unknown(version: u32) -> BinError {
    BinError::new(
        0,
        format!("unknown w3i version {version} (18, 25, 28 and 31 are known)"),
    )
}

/// Reads a `war3map.w3i`. Every byte must be read.
pub fn parse(bytes: &[u8]) -> BinResult<Info> {
    let mut r = Reader::new(bytes);
    let version = r.u32()?;
    if !VERSIONS.contains(&version) {
        return Err(unknown(version));
    }
    let has = Has::of(version);
    let mut i = Info::new(version);
    i.saves = r.u32()?;
    i.editor_version = r.u32()?;
    if has.v28 {
        for n in &mut i.game_version {
            *n = r.u32()?;
        }
    }
    i.name = r.cstr()?;
    i.author = r.cstr()?;
    i.description = r.cstr()?;
    i.players_recommended = r.cstr()?;
    for f in &mut i.camera_bounds {
        *f = r.f32()?;
    }
    for n in &mut i.camera_complements {
        *n = r.i32()?;
    }
    i.playable_width = r.i32()?;
    i.playable_height = r.i32()?;
    i.flags = r.u32()?;
    i.tileset = r.u8()?;
    i.loading_screen = r.i32()?;
    if has.tft {
        i.loading_screen_model = r.cstr()?;
    }
    i.loading_text = r.cstr()?;
    i.loading_title = r.cstr()?;
    i.loading_subtitle = r.cstr()?;
    i.game_data_set = r.i32()?;
    if has.tft {
        i.prologue_path = r.cstr()?;
    }
    i.prologue_text = r.cstr()?;
    i.prologue_title = r.cstr()?;
    i.prologue_subtitle = r.cstr()?;
    if has.tft {
        i.fog = Fog {
            style: r.i32()?,
            start_z: r.f32()?,
            end_z: r.f32()?,
            density: r.f32()?,
            color: r.array()?,
        };
        i.weather = r.rawcode()?;
        i.sound_environment = r.cstr()?;
        i.light_tileset = r.u8()?;
        i.water_color = r.array()?;
    }
    if has.v28 {
        i.script = r.u32()?;
    }
    if has.v31 {
        i.supported_modes = r.u32()?;
        i.game_data_version = r.u32()?;
    }

    let player_size = if has.v31 { 41 } else { 33 };
    i.players = r.list(player_size, |r| {
        let mut p = Player {
            number: r.i32()?,
            kind: r.i32()?,
            race: r.i32()?,
            fixed_start: r.i32()?,
            name: r.cstr()?,
            start_x: r.f32()?,
            start_y: r.f32()?,
            ally_low: r.u32()?,
            ally_high: r.u32()?,
            ..Player::default()
        };
        if has.v31 {
            p.enemy_low = r.u32()?;
            p.enemy_high = r.u32()?;
        }
        Ok(p)
    })?;
    i.forces = r.list(9, |r| {
        Ok(Force {
            flags: r.u32()?,
            players: r.u32()?,
            name: r.cstr()?,
        })
    })?;
    // A cut file: fewer bytes left than a count takes, where a list starts.
    let cut = |r: &mut Reader, after: Part| {
        (r.remaining() < 4).then(|| Cut {
            after,
            bytes: r.rest().to_vec(),
        })
    };
    if let Some(c) = cut(&mut r, Part::Forces) {
        i.cut = Some(c);
        return Ok(i);
    }
    i.upgrades = r.list(16, |r| {
        Ok(Upgrade {
            players: r.u32()?,
            id: r.rawcode()?,
            level: r.i32()?,
            availability: r.i32()?,
        })
    })?;
    if let Some(c) = cut(&mut r, Part::Upgrades) {
        i.cut = Some(c);
        return Ok(i);
    }
    i.techs = r.list(8, |r| {
        Ok(Tech {
            players: r.u32()?,
            id: r.rawcode()?,
        })
    })?;
    if let Some(c) = cut(&mut r, Part::Techs) {
        i.cut = Some(c);
        return Ok(i);
    }
    i.unit_tables = r.list(13, |r| {
        let number = r.i32()?;
        let name = r.cstr()?;
        let columns = r.list(4, |r| r.i32())?;
        let n = columns.len();
        let rows = r.list(4 + 4 * n, |r| {
            let chance = r.i32()?;
            let ids = (0..n).map(|_| r.rawcode()).collect::<BinResult<_>>()?;
            Ok((chance, ids))
        })?;
        Ok(UnitTable {
            number,
            name,
            columns,
            rows,
        })
    })?;
    if has.tft {
        if let Some(c) = cut(&mut r, Part::UnitTables) {
            i.cut = Some(c);
            return Ok(i);
        }
        i.item_tables = r.list(9, |r| {
            Ok(ItemTable {
                number: r.i32()?,
                name: r.cstr()?,
                sets: r.list(4, |r| r.list(8, |r| Ok((r.i32()?, r.rawcode()?))))?,
            })
        })?;
    }
    r.finish()?;
    Ok(i)
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
