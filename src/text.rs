//! The text form of a map info file, for people to read and edit: BNI
//! (INI as the game reads it, `bni-rs`). A `[w3i]` section with the map's
//! own fields, then a section per player, force, upgrade, tech, random
//! unit table and random item table, in file order.
//!
//! ```ini
//! [w3i]
//! version=25 // The Frozen Throne
//! saves=12
//! name="TRIGSTR_001"
//! flags=0x00008C14 // melee map, ...
//! tileset=L // Lordaeron Summer
//! ...
//!
//! [player]
//! number=0 // Player 1
//! type=1 // human
//! race=1 // human
//! name="TRIGSTR_008"
//! ...
//!
//! [force]
//! flags=0x00000003 // allied, allied victory
//! players=0x00000003 // 1, 2
//! name="TRIGSTR_011"
//! ```
//!
//! Strings are quoted, with escapes for what BNI can't hold
//! ([`bni_rs::string`]). Player masks and flags are hex; the comment after
//! a value says what it means and isn't read. Numbers are written as the
//! shortest decimal that reads back as the same `f32`, so the text gives
//! back the exact bytes. A field the version doesn't have is an error; one
//! left out takes its default ([`Info::new`]).

use bni_rs::Document;
use bni_rs::ast::{Field, Item, Section};
use bni_rs::string::{quote, unquote};
use parser_rs::{ParseError, Span};

use crate::{
    Cut, FLAGS, FORCE_FLAGS, Force, Has, Info, ItemTable, Part, Player, Rawcode, Tech, UnitTable,
    Upgrade, VERSIONS, describe,
};

/// The first section's name.
pub const HEADER: &str = "w3i";

// ---------------------------------------------------------------- writing

/// Lines of `key=value`, with a comment after the value when there's one.
struct Out(String);

impl Out {
    fn line(&mut self, key: &str, value: impl std::fmt::Display, note: Option<String>) {
        match note.filter(|n| !n.is_empty()) {
            Some(n) => self.0 += &format!("{key}={value} // {n}\n"),
            None => self.0 += &format!("{key}={value}\n"),
        }
    }
    fn str(&mut self, key: &str, s: &str) {
        self.line(key, quote(s), None);
    }
    fn section(&mut self, name: &str) {
        self.0 += &format!("\n[{name}]\n");
    }
}

fn hex(n: u32) -> String {
    format!("0x{n:08X}")
}

/// A float: its shortest decimal, or its bits for a NaN.
fn real(v: f32) -> String {
    if v.is_nan() {
        format!("0x{:08X}", v.to_bits())
    } else {
        v.to_string()
    }
}

fn reals(list: &[f32]) -> String {
    list.iter().map(|&v| real(v)).collect::<Vec<_>>().join(",")
}

fn ints<T: std::fmt::Display>(list: &[T]) -> String {
    list.iter().map(T::to_string).collect::<Vec<_>>().join(",")
}

/// A rawcode: its four characters when they are letters, digits or `_`,
/// else `0x` and hex.
fn code(c: Rawcode) -> String {
    if c.0.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_') {
        c.to_string()
    } else {
        format!("0x{:08X}", c.to_int())
    }
}

/// A letter, or a number for any other byte.
fn letter(b: u8) -> String {
    if b.is_ascii_alphabetic() {
        (b as char).to_string()
    } else {
        b.to_string()
    }
}

/// The names of the bits set, and what's left unnamed in hex.
fn bits(n: u32, names: &[(u32, &str)]) -> String {
    let mut out: Vec<String> = names
        .iter()
        .filter(|(b, _)| n & b != 0)
        .map(|(_, name)| name.to_string())
        .collect();
    let rest = names.iter().fold(n, |n, (b, _)| n & !b);
    if rest != 0 {
        out.push(format!("other bits 0x{rest:X}"));
    }
    out.join(", ")
}

/// The players of a mask, counting from 1.
fn players(mask: u32) -> String {
    (0..32)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| (i + 1).to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn named(n: i64, names: &[&str]) -> Option<String> {
    usize::try_from(n)
        .ok()
        .and_then(|i| names.get(i))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

const PLAYER_TYPES: [&str; 5] = ["", "human", "computer", "neutral", "rescuable"];
const RACES: [&str; 5] = ["selectable", "human", "orc", "undead", "night elf"];
const COLUMNS: [&str; 3] = ["unit", "building", "item"];
const FOG_STYLES: [&str; 4] = ["none", "linear", "exponential", "exponential squared"];

/// The base tilesets, by letter (as `w3e-rs` has them).
const TILESETS: [(u8, &str); 18] = [
    (b'A', "Ashenvale"),
    (b'B', "Barrens"),
    (b'K', "Black Citadel"),
    (b'Y', "Cityscape"),
    (b'X', "Dalaran"),
    (b'J', "Dalaran Ruins"),
    (b'D', "Dungeon"),
    (b'C', "Felwood"),
    (b'I', "Icecrown Glacier"),
    (b'F', "Lordaeron Fall"),
    (b'L', "Lordaeron Summer"),
    (b'W', "Lordaeron Winter"),
    (b'N', "Northrend"),
    (b'O', "Outland"),
    (b'Z', "Sunken Ruins"),
    (b'G', "Underground"),
    (b'V', "Village"),
    (b'Q', "Village Fall"),
];

fn tileset_name(letter: u8) -> Option<String> {
    TILESETS
        .iter()
        .find(|(l, _)| *l == letter)
        .map(|(_, n)| n.to_string())
}

/// The text form of `info`.
pub fn to_text(i: &Info) -> String {
    let has = i.has();
    let mut o = Out(String::new());
    o.0 += "// Warcraft III map info (war3map.w3i): the Scenario menu's settings.\n";
    o.0 += &format!("[{HEADER}]\n");
    o.line("version", i.version, Some(describe(i.version).into()));
    o.line("saves", i.saves, None);
    o.line("editor_version", i.editor_version, None);
    if has.v28 {
        o.line(
            "game_version",
            ints(&i.game_version),
            Some("major, minor, patch, build".into()),
        );
    }
    o.str("name", &i.name);
    o.str("author", &i.author);
    o.str("description", &i.description);
    o.str("players_recommended", &i.players_recommended);
    o.line(
        "camera_bounds",
        reals(&i.camera_bounds),
        Some("x,y: bottom left, top right, top left, bottom right".into()),
    );
    o.line(
        "camera_complements",
        ints(&i.camera_complements),
        Some("left, right, bottom, top".into()),
    );
    o.line("playable_width", i.playable_width, None);
    o.line("playable_height", i.playable_height, None);
    o.line("flags", hex(i.flags), Some(bits(i.flags, &FLAGS)));
    o.line("tileset", letter(i.tileset), tileset_name(i.tileset));
    o.line("loading_screen", i.loading_screen, None);
    if has.tft {
        o.str("loading_screen_model", &i.loading_screen_model);
    }
    o.str("loading_text", &i.loading_text);
    o.str("loading_title", &i.loading_title);
    o.str("loading_subtitle", &i.loading_subtitle);
    if has.tft {
        o.line("game_data_set", i.game_data_set, None);
        o.str("prologue_path", &i.prologue_path);
    } else {
        o.line("prologue_screen", i.prologue_screen, None);
    }
    o.str("prologue_text", &i.prologue_text);
    o.str("prologue_title", &i.prologue_title);
    o.str("prologue_subtitle", &i.prologue_subtitle);
    if has.tft {
        o.line(
            "fog_style",
            i.fog.style,
            named(i.fog.style.into(), &FOG_STYLES),
        );
        o.line("fog_start_z", real(i.fog.start_z), None);
        o.line("fog_end_z", real(i.fog.end_z), None);
        o.line("fog_density", real(i.fog.density), None);
        o.line(
            "fog_color",
            ints(&i.fog.color),
            Some("red, green, blue, alpha".into()),
        );
        o.line(
            "weather",
            code(i.weather),
            i.weather.is_none().then(|| "none".into()),
        );
        o.str("sound_environment", &i.sound_environment);
        o.line(
            "light_tileset",
            letter(i.light_tileset),
            match i.light_tileset {
                0 => Some("the map's own".into()),
                t => tileset_name(t),
            },
        );
        o.line(
            "water_color",
            ints(&i.water_color),
            Some("red, green, blue, alpha".into()),
        );
    }
    if has.v28 {
        o.line("script", i.script, named(i.script.into(), &["JASS", "Lua"]));
    }
    if has.v31 {
        o.line(
            "supported_modes",
            i.supported_modes,
            named(i.supported_modes.into(), &["", "SD", "HD", "SD and HD"]),
        );
        o.line("game_data_version", i.game_data_version, None);
    }
    if let Some(cut) = &i.cut {
        o.line(
            "cut",
            cut.after.name(),
            Some("the file stops after these, as a map optimizer left it".into()),
        );
        o.line("cut_bytes", ints(&cut.bytes), None);
    }

    for p in &i.players {
        o.section("player");
        o.line(
            "number",
            p.number,
            Some(format!("Player {}", i64::from(p.number) + 1)),
        );
        o.line("type", p.kind, named(p.kind.into(), &PLAYER_TYPES));
        o.line("race", p.race, named(p.race.into(), &RACES));
        o.line("fixed_start", p.fixed_start, None);
        o.str("name", &p.name);
        o.line("start_x", real(p.start_x), None);
        o.line("start_y", real(p.start_y), None);
        o.line("ally_low", hex(p.ally_low), Some(players(p.ally_low)));
        o.line("ally_high", hex(p.ally_high), Some(players(p.ally_high)));
        if has.v31 {
            o.line("enemy_low", hex(p.enemy_low), Some(players(p.enemy_low)));
            o.line("enemy_high", hex(p.enemy_high), Some(players(p.enemy_high)));
        }
    }
    for f in &i.forces {
        o.section("force");
        o.line("flags", hex(f.flags), Some(bits(f.flags, &FORCE_FLAGS)));
        o.line("players", hex(f.players), Some(players(f.players)));
        o.str("name", &f.name);
    }
    for u in &i.upgrades {
        o.section("upgrade");
        o.line("players", hex(u.players), Some(players(u.players)));
        o.line("id", code(u.id), None);
        o.line("level", u.level, None);
        o.line(
            "availability",
            u.availability,
            named(
                u.availability.into(),
                &["unavailable", "available", "researched"],
            ),
        );
    }
    for t in &i.techs {
        o.section("tech");
        o.line("players", hex(t.players), Some(players(t.players)));
        o.line("id", code(t.id), None);
    }
    for t in &i.unit_tables {
        o.section("unit_table");
        o.line("number", t.number, None);
        o.str("name", &t.name);
        let kinds: Vec<String> = t
            .columns
            .iter()
            .map(|&c| named(c.into(), &COLUMNS).unwrap_or_else(|| c.to_string()))
            .collect();
        o.line("columns", ints(&t.columns), Some(kinds.join(", ")));
        for (chance, ids) in &t.rows {
            let mut value = chance.to_string();
            for &id in ids {
                value += ",";
                value += &code(id);
            }
            o.line("row", value, None);
        }
    }
    for t in &i.item_tables {
        o.section("item_table");
        o.line("number", t.number, None);
        o.str("name", &t.name);
        for set in &t.sets {
            let value: Vec<String> = set
                .iter()
                .map(|&(chance, id)| format!("{chance},{}", code(id)))
                .collect();
            o.line("set", value.join(","), None);
        }
    }
    o.0
}

// ---------------------------------------------------------------- reading

/// Is `doc` a map info file's text form: is its first section `[w3i]`?
pub fn is_info_text(doc: &Document) -> bool {
    doc.sections.first().is_some_and(|s| s.name.text == HEADER)
}

/// What [`from_text`] read: the file when the text has no errors, and
/// every error, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub info: Option<Info>,
    pub errors: Vec<ParseError>,
}

/// Reads the text form back.
pub fn from_text(text: &str) -> Parsed {
    from_bni(&bni_rs::parse(text))
}

/// [`from_text`] of a document already parsed. Its BNI errors come first.
pub fn from_bni(doc: &Document) -> Parsed {
    let mut e = Errors(doc.errors.clone());
    for item in &doc.preamble {
        e.at(
            item_span(item),
            format!("a field outside a section: the file starts with [{HEADER}]"),
        );
    }
    let Some((head, rest)) = doc
        .sections
        .split_first()
        .filter(|(h, _)| h.name.text == HEADER)
    else {
        let span = doc
            .sections
            .first()
            .map_or(Span::new(0, 0, 0, 1), |s| s.name.span);
        e.at(span, format!("the file starts with [{HEADER}]"));
        return Parsed {
            info: None,
            errors: e.0,
        };
    };
    let mut info = header(head, &mut e);
    let has = info.has();
    let version = info.version;
    for section in rest {
        let name = section.name.text.as_str();
        // The list the section is in, for a cut file.
        let part = match name {
            "player" | "force" => Part::Forces,
            "upgrade" => Part::Upgrades,
            "tech" => Part::Techs,
            "unit_table" | "item_table" => Part::UnitTables,
            _ => {
                e.at(
                    section.name.span,
                    format!(
                        "no section [{name}]: after [{HEADER}] come [player], [force], [upgrade], [tech], [unit_table], [item_table]"
                    ),
                );
                continue;
            }
        };
        if name == "item_table" && !has.tft {
            e.at(
                section.name.span,
                format!("random item tables are in version 25 (The Frozen Throne) and later; this file is {version}"),
            );
            continue;
        }
        if let Some(cut) = &info.cut
            && (part > cut.after || name == "item_table")
        {
            e.at(
                section.name.span,
                format!(
                    "the file is cut after its {} (`cut` in [{HEADER}]): nothing comes after them",
                    cut.after.name()
                ),
            );
        }
        let mut f = Fields::new(section, &mut e);
        match name {
            "player" => {
                let mut p = Player::default();
                f.int("number", &mut p.number, &mut e);
                f.int("type", &mut p.kind, &mut e);
                f.int("race", &mut p.race, &mut e);
                f.int("fixed_start", &mut p.fixed_start, &mut e);
                f.string("name", &mut p.name, &mut e);
                f.real("start_x", &mut p.start_x, &mut e);
                f.real("start_y", &mut p.start_y, &mut e);
                f.mask("ally_low", &mut p.ally_low, &mut e);
                f.mask("ally_high", &mut p.ally_high, &mut e);
                for (key, out) in [
                    ("enemy_low", &mut p.enemy_low),
                    ("enemy_high", &mut p.enemy_high),
                ] {
                    if f.since(has.v31, key, 31, version, &mut e) {
                        f.mask(key, out, &mut e);
                    }
                }
                info.players.push(p);
            }
            "force" => {
                let mut force = Force::default();
                f.mask("flags", &mut force.flags, &mut e);
                f.mask("players", &mut force.players, &mut e);
                f.string("name", &mut force.name, &mut e);
                info.forces.push(force);
            }
            "upgrade" => {
                let mut u = Upgrade::default();
                f.mask("players", &mut u.players, &mut e);
                f.code("id", &mut u.id, &mut e);
                f.int("level", &mut u.level, &mut e);
                f.int("availability", &mut u.availability, &mut e);
                info.upgrades.push(u);
            }
            "tech" => {
                let mut t = Tech::default();
                f.mask("players", &mut t.players, &mut e);
                f.code("id", &mut t.id, &mut e);
                info.techs.push(t);
            }
            "unit_table" => {
                let mut t = UnitTable::default();
                f.int("number", &mut t.number, &mut e);
                f.string("name", &mut t.name, &mut e);
                if let Some((_, columns)) = f.values("columns", &mut e, int) {
                    t.columns = columns;
                }
                let n = t.columns.len();
                for item in f.all("row") {
                    match row(item) {
                        Ok((chance, ids)) if ids.len() == n => t.rows.push((chance, ids)),
                        Ok((_, ids)) => e.at(
                            item_span(item),
                            format!(
                                "a row is a chance, then an id per column: {} ids for {n} columns",
                                ids.len()
                            ),
                        ),
                        Err(m) => e.at(item_span(item), m),
                    }
                }
                info.unit_tables.push(t);
            }
            _ => {
                let mut t = ItemTable::default();
                f.int("number", &mut t.number, &mut e);
                f.string("name", &mut t.name, &mut e);
                for item in f.all("set") {
                    match set(item) {
                        Ok(s) => t.sets.push(s),
                        Err(m) => e.at(item_span(item), m),
                    }
                }
                info.item_tables.push(t);
            }
        }
        f.finish(&mut e);
    }
    // In line order, as fields are read in the file's order instead.
    let mut errors = e.0;
    errors.sort_by_key(|e| (e.line, e.col));
    let info = errors.is_empty().then_some(info);
    Parsed { info, errors }
}

/// The `[w3i]` section: its version first, which says what else it has.
fn header(section: &Section, e: &mut Errors) -> Info {
    let mut f = Fields::new(section, e);
    let mut version = 25;
    if let Some(item) = f.one("version", e) {
        match single(item).and_then(|v| v.text().trim().parse::<u32>().ok()) {
            Some(v) if VERSIONS.contains(&v) => version = v,
            _ => e.at(
                item_span(item),
                "the version is 18 (Reign of Chaos), 25 (The Frozen Throne), 28 (1.31) or 31 (1.32 and later)",
            ),
        }
    }
    let has = Has::of(version);
    let mut i = Info::new(version);
    f.mask("saves", &mut i.saves, e);
    f.mask("editor_version", &mut i.editor_version, e);
    if f.since(has.v28, "game_version", 28, version, e) {
        f.array("game_version", &mut i.game_version, e, mask);
    }
    f.string("name", &mut i.name, e);
    f.string("author", &mut i.author, e);
    f.string("description", &mut i.description, e);
    f.string("players_recommended", &mut i.players_recommended, e);
    f.array("camera_bounds", &mut i.camera_bounds, e, float);
    f.array("camera_complements", &mut i.camera_complements, e, int);
    f.int("playable_width", &mut i.playable_width, e);
    f.int("playable_height", &mut i.playable_height, e);
    f.mask("flags", &mut i.flags, e);
    f.one_of("tileset", &mut i.tileset, e, byte_letter);
    f.int("loading_screen", &mut i.loading_screen, e);
    if f.since(has.tft, "loading_screen_model", 25, version, e) {
        f.string("loading_screen_model", &mut i.loading_screen_model, e);
    }
    f.string("loading_text", &mut i.loading_text, e);
    f.string("loading_title", &mut i.loading_title, e);
    f.string("loading_subtitle", &mut i.loading_subtitle, e);
    if f.since(has.tft, "game_data_set", 25, version, e) {
        f.int("game_data_set", &mut i.game_data_set, e);
    }
    if f.since(has.tft, "prologue_path", 25, version, e) {
        f.string("prologue_path", &mut i.prologue_path, e);
    }
    if !has.tft {
        f.int("prologue_screen", &mut i.prologue_screen, e);
    } else if let Some(item) = f.one("prologue_screen", e) {
        e.at(
            item_span(item),
            "`prologue_screen` is only in version 18 (Reign of Chaos); later ones have `prologue_path`",
        );
    }
    f.string("prologue_text", &mut i.prologue_text, e);
    f.string("prologue_title", &mut i.prologue_title, e);
    f.string("prologue_subtitle", &mut i.prologue_subtitle, e);
    for key in [
        "fog_style",
        "fog_start_z",
        "fog_end_z",
        "fog_density",
        "fog_color",
        "weather",
        "sound_environment",
        "light_tileset",
        "water_color",
    ] {
        if !f.since(has.tft, key, 25, version, e) {
            continue;
        }
        match key {
            "fog_style" => f.int(key, &mut i.fog.style, e),
            "fog_start_z" => f.real(key, &mut i.fog.start_z, e),
            "fog_end_z" => f.real(key, &mut i.fog.end_z, e),
            "fog_density" => f.real(key, &mut i.fog.density, e),
            "fog_color" => f.array(key, &mut i.fog.color, e, byte),
            "weather" => f.code(key, &mut i.weather, e),
            "sound_environment" => f.string(key, &mut i.sound_environment, e),
            "light_tileset" => f.one_of(key, &mut i.light_tileset, e, byte_letter),
            _ => f.array(key, &mut i.water_color, e, byte),
        }
    }
    if f.since(has.v28, "script", 28, version, e) {
        f.mask("script", &mut i.script, e);
    }
    for (key, out) in [
        ("supported_modes", &mut i.supported_modes),
        ("game_data_version", &mut i.game_data_version),
    ] {
        if f.since(has.v31, key, 31, version, e) {
            f.mask(key, out, e);
        }
    }
    let mut after = None;
    if let Some(item) = f.one("cut", e) {
        match single(item).and_then(|v| Part::from_name(v.text().trim())) {
            Some(p) => after = Some(p),
            None => e.at(
                item_span(item),
                "the file is cut after its forces, upgrades, techs or unit_tables",
            ),
        }
    }
    let bytes = f.values("cut_bytes", e, byte);
    match (after, bytes) {
        (_, Some((span, bytes))) if bytes.len() > 3 => {
            e.at(span, "at most 3 `cut_bytes`: 4 would be a count")
        }
        (Some(after), bytes) => {
            i.cut = Some(Cut {
                after,
                bytes: bytes.map(|(_, b)| b).unwrap_or_default(),
            })
        }
        (None, Some((span, _))) => e.at(span, "`cut_bytes` are for a file that's `cut`"),
        (None, None) => {}
    }
    f.finish(e);
    i
}

struct Errors(Vec<ParseError>);

impl Errors {
    fn at(&mut self, span: Span, message: impl Into<String>) {
        self.0.push(error(span, message));
    }
}

/// A section's items, taken by key; what's left at the end is unknown.
struct Fields<'a> {
    section: &'a str,
    items: Vec<(&'a str, &'a Item)>,
    used: Vec<bool>,
    known: Vec<&'static str>,
}

impl<'a> Fields<'a> {
    fn new(section: &'a Section, e: &mut Errors) -> Fields<'a> {
        let mut items = Vec::new();
        for item in &section.items {
            match &item.key {
                Some(k) => items.push((k.text.trim(), item)),
                None => e.at(item_span(item), "a field without a name"),
            }
        }
        let used = vec![false; items.len()];
        Fields {
            section: &section.name.text,
            items,
            used,
            known: Vec::new(),
        }
    }

    /// Every item with `key`, in order, now used.
    fn all(&mut self, key: &'static str) -> Vec<&'a Item> {
        if !self.known.contains(&key) {
            self.known.push(key);
        }
        let mut out = Vec::new();
        for (n, (k, item)) in self.items.iter().enumerate() {
            if *k == key {
                self.used[n] = true;
                out.push(*item);
            }
        }
        out
    }

    /// The item with `key`; a second one is an error.
    fn one(&mut self, key: &'static str, e: &mut Errors) -> Option<&'a Item> {
        let all = self.all(key);
        for item in all.iter().skip(1) {
            e.at(item_span(item), format!("`{key}` is set twice"));
        }
        all.first().copied()
    }

    /// Whether to read `key`: when the version `has` it. When it doesn't,
    /// the field is an error.
    fn since(
        &mut self,
        has: bool,
        key: &'static str,
        from: u32,
        version: u32,
        e: &mut Errors,
    ) -> bool {
        if !has && let Some(item) = self.one(key, e) {
            e.at(
                item_span(item),
                format!(
                    "`{key}` is in version {from} ({}) and later; this file is {version}",
                    describe(from)
                ),
            );
        }
        has
    }

    fn one_of<T>(
        &mut self,
        key: &'static str,
        out: &mut T,
        e: &mut Errors,
        read: impl Fn(&Field) -> Result<T, String>,
    ) {
        if let Some(item) = self.one(key, e) {
            match single(item).ok_or("one value".to_string()).and_then(&read) {
                Ok(v) => *out = v,
                Err(m) => e.at(item_span(item), format!("`{key}`: {m}")),
            }
        }
    }
    fn int(&mut self, key: &'static str, out: &mut i32, e: &mut Errors) {
        self.one_of(key, out, e, int)
    }
    fn mask(&mut self, key: &'static str, out: &mut u32, e: &mut Errors) {
        self.one_of(key, out, e, mask)
    }
    fn real(&mut self, key: &'static str, out: &mut f32, e: &mut Errors) {
        self.one_of(key, out, e, float)
    }
    fn code(&mut self, key: &'static str, out: &mut Rawcode, e: &mut Errors) {
        self.one_of(key, out, e, rawcode)
    }
    fn string(&mut self, key: &'static str, out: &mut String, e: &mut Errors) {
        let Some(item) = self.one(key, e) else { return };
        match item.fields.as_slice() {
            [] => out.clear(),
            [f] => match unquote(f) {
                Ok(s) => *out = s,
                Err(err) => e.0.push(err),
            },
            _ => e.at(
                item_span(item),
                format!("`{key}` is one string: a comma goes inside the quotes"),
            ),
        }
    }

    /// The comma-separated values of `key`, and its span; `None` when it's
    /// not there or a value is wrong (an error).
    fn values<T>(
        &mut self,
        key: &'static str,
        e: &mut Errors,
        read: impl Fn(&Field) -> Result<T, String>,
    ) -> Option<(Span, Vec<T>)> {
        let item = self.one(key, e)?;
        let span = item_span(item);
        if item.value_text().trim().is_empty() {
            return Some((span, Vec::new()));
        }
        let mut values = Vec::new();
        for f in &item.fields {
            match read(f) {
                Ok(v) => values.push(v),
                Err(m) => {
                    e.at(f.span, format!("`{key}`: {m}"));
                    return None;
                }
            }
        }
        Some((span, values))
    }

    fn array<T: Copy, const N: usize>(
        &mut self,
        key: &'static str,
        out: &mut [T; N],
        e: &mut Errors,
        read: impl Fn(&Field) -> Result<T, String>,
    ) {
        if let Some((span, values)) = self.values(key, e, read) {
            match <[T; N]>::try_from(values.as_slice()) {
                Ok(a) => *out = a,
                Err(_) => e.at(
                    span,
                    format!("`{key}` has {N} values, comma-separated, not {}", values.len()),
                ),
            }
        }
    }

    /// Unknown keys are errors.
    fn finish(self, e: &mut Errors) {
        for (n, (k, item)) in self.items.iter().enumerate() {
            if !self.used[n] {
                e.at(
                    item_span(item),
                    format!(
                        "no field `{k}` in [{}]: it has {}",
                        self.section,
                        self.known.join(", ")
                    ),
                );
            }
        }
    }
}

fn single(item: &Item) -> Option<&Field> {
    match item.fields.as_slice() {
        [f] => Some(f),
        _ => None,
    }
}

fn int(f: &Field) -> Result<i32, String> {
    f.text()
        .trim()
        .parse()
        .map_err(|_| "a whole number".into())
}

/// Decimal, or `0x` and hex.
fn mask(f: &Field) -> Result<u32, String> {
    let t = f.text();
    let t = t.trim();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u32::from_str_radix(h, 16),
        None => t.parse(),
    }
    .map_err(|_| "a whole number, 0 or more (or 0x and hex)".into())
}

fn byte(f: &Field) -> Result<u8, String> {
    f.text()
        .trim()
        .parse()
        .map_err(|_| "a number from 0 to 255".into())
}

/// A letter, or a number for any other byte.
fn byte_letter(f: &Field) -> Result<u8, String> {
    let t = f.text();
    let t = t.trim();
    match t.as_bytes() {
        [c] if c.is_ascii_alphabetic() => Ok(*c),
        _ => t
            .parse()
            .map_err(|_| "a letter, or a number from 0 to 255".into()),
    }
}

/// A number, or `0x` and its bits (a NaN).
fn float(f: &Field) -> Result<f32, String> {
    let t = f.text();
    let t = t.trim();
    match t.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16).map(f32::from_bits).ok(),
        None => t.parse().ok(),
    }
    .ok_or_else(|| "a number".into())
}

fn rawcode(f: &Field) -> Result<Rawcode, String> {
    let t = f.text();
    Rawcode::parse(t.trim()).ok_or_else(|| {
        format!(
            "`{}` isn't an id: four characters, or 0x and eight hex digits",
            t.trim()
        )
    })
}

/// `row=chance,id,id,...`.
fn row(item: &Item) -> Result<(i32, Vec<Rawcode>), String> {
    let (first, ids) = item
        .fields
        .split_first()
        .ok_or("a row is a chance, then an id per column")?;
    let chance = int(first)?;
    let ids = ids.iter().map(rawcode).collect::<Result<_, _>>()?;
    Ok((chance, ids))
}

/// `set=chance,id,chance,id,...`.
fn set(item: &Item) -> Result<Vec<(i32, Rawcode)>, String> {
    if item.value_text().trim().is_empty() {
        return Ok(Vec::new());
    }
    if !item.fields.len().is_multiple_of(2) {
        return Err("a set is pairs: a chance, then an item id".into());
    }
    item.fields
        .chunks(2)
        .map(|p| Ok((int(&p[0])?, rawcode(&p[1])?)))
        .collect()
}

fn error(span: Span, message: impl Into<String>) -> ParseError {
    let end_col = if span.end_line == span.start_line {
        span.end_col.max(span.start_col + 1)
    } else {
        span.start_col + 1
    };
    ParseError {
        message: message.into(),
        line: span.start_line,
        col: span.start_col,
        end_col,
    }
}

fn item_span(item: &Item) -> Span {
    let start = item.key.as_ref().map_or(item.eq, |k| k.span);
    let end = item.fields.last().map_or(item.eq, |f| f.span);
    Span::new(start.start_line, start.start_col, end.end_line, end.end_col)
}

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
