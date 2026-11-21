use super::*;

/// A file in `version` with every list and awkward values.
pub(crate) fn full(version: u32) -> Info {
    let has = Has::of(version);
    let mut i = Info::new(version);
    i.saves = 12;
    i.editor_version = 6059;
    if has.v28 {
        i.game_version = [1, 31, 1, 12164];
    }
    i.name = "TRIGSTR_001".into();
    i.author = "Me, \"quoted\"".into();
    i.description = "Line one\nLine two // not a comment".into();
    i.players_recommended = String::new();
    i.camera_bounds = [-3328.0, -3584.0, 3328.0, 3072.0, -3328.0, 3072.0, 3328.0, -3584.0];
    i.camera_complements = [6, 6, 4, 8];
    i.playable_width = 84;
    i.playable_height = 84;
    i.flags = 0x8C14 | 0x8000_0000;
    i.tileset = b'L';
    if has.tft {
        i.loading_screen_model = "Loading\\Screen.mdx".into();
        i.prologue_path = "x".into();
        i.game_data_set = 1;
        i.fog.style = 1;
        i.fog.start_z = 3000.0;
        i.fog.end_z = 5000.0;
        i.fog.density = 0.5;
        i.fog.color = [10, 20, 30, 255];
        i.weather = Rawcode(*b"RAhr");
        i.sound_environment = "cave".into();
        i.light_tileset = b'A';
        i.water_color = [255, 255, 255, 0];
    } else {
        i.prologue_screen = 2;
    }
    if has.v28 {
        i.script = 1;
    }
    if has.v31 {
        i.supported_modes = 3;
        i.game_data_version = 1;
    }
    i.loading_text = "Welcome".into();
    i.players = vec![Player {
        number: 0,
        kind: 1,
        race: 2,
        fixed_start: 1,
        name: "TRIGSTR_008".into(),
        start_x: -1856.5,
        start_y: f32::from_bits(0x7FC0_0001),
        ally_low: 2,
        ally_high: 0,
        enemy_low: if has.v31 { 4 } else { 0 },
        enemy_high: 0,
    }];
    i.forces = vec![Force { flags: 0x0B, players: 3, name: "Team, one".into() }];
    i.upgrades = vec![Upgrade { players: u32::MAX, id: Rawcode(*b"Rhme"), level: 1, availability: 2 }];
    i.techs = vec![Tech { players: 1, id: Rawcode(*b"hfoo") }];
    i.unit_tables = vec![UnitTable {
        number: 0,
        name: "Group".into(),
        columns: vec![0, 2],
        rows: vec![(50, vec![Rawcode(*b"hfoo"), Rawcode([0; 4])]), (50, vec![Rawcode(*b"YYU1"), Rawcode(*b"ratf")])],
    }];
    if has.tft {
        i.item_tables = vec![ItemTable {
            number: 3,
            name: "Drops".into(),
            sets: vec![vec![(10, Rawcode(*b"ratf")), (90, Rawcode(*b"rde1"))], vec![]],
        }];
    }
    i
}

#[test]
fn reads_and_writes_back_every_version() {
    for v in VERSIONS {
        let i = full(v);
        let bytes = i.to_bytes().unwrap();
        let back = parse(&bytes).unwrap();
        assert_eq!(back.to_bytes().unwrap(), bytes);
        assert_eq!(back.players[0].enemy_low, if v >= 31 { 4 } else { 0 });
        assert_eq!(back.item_tables.len(), usize::from(v >= 25));
        assert_eq!(back.game_version[0], u32::from(v >= 28));
    }
    // What each version adds: 1.31 the game version and script, 1.32 two
    // fields and the enemy priorities.
    let len = |v| full(v).to_bytes().unwrap().len();
    let mut v28 = full(28);
    v28.version = 25;
    v28.script = 0;
    v28.game_version = [0; 4];
    assert_eq!(len(28), v28.to_bytes().unwrap().len() + 16 + 4);
    assert_eq!(len(31), len(28) + 8 + 8);
}

#[test]
fn bad_files() {
    let err = |bytes: &[u8]| parse(bytes).unwrap_err().to_string();
    assert!(err(&24u32.to_le_bytes()).contains("unknown w3i version 24"));
    let bytes = full(25).to_bytes().unwrap();
    assert!(err(&bytes[..40]).contains("end of"), "{}", err(&bytes[..40]));
    let mut more = bytes.clone();
    more.extend([1, 2, 3, 4, 5]);
    assert!(err(&more).contains("5 extra bytes"));
    // A cut file can't have lists after its cut.
    let mut i = full(25);
    i.cut = Some(Cut { after: Part::Upgrades, bytes: vec![] });
    assert!(i.to_bytes().unwrap_err().to_string().contains("cut after its upgrades"));
    let mut i = full(25);
    i.unit_tables[0].rows[0].1.pop();
    assert!(i.to_bytes().unwrap_err().to_string().contains("1 ids for 2 columns"));
}

#[test]
fn flags() {
    assert_eq!(flag_names(0x8004), ["melee map", "use item classification system"]);
    let (set, rest) = bits(&FORCE_FLAGS, 0x4B);
    assert_eq!(set.iter().map(|m| m.2).collect::<Vec<_>>(), ["WESTRING_ALLIED", "WESTRING_ALLIEDVICTORY", "WESTRING_SHAREVISION"]);
    assert_eq!(rest, 0x40);
    assert_eq!(meaning(&RACES, 4), Some((4, "night elf", "WESTRING_RACE_NIGHTELF")));
    assert_eq!(meaning(&RACES, 9), None);
    assert_eq!(describe(31), "1.32 and later");
}

/// Every map's info reads and writes back to the same bytes: `W3I_CORPUS`
/// is a folder of `<map>/war3map.w3i`.
#[test]
#[ignore]
fn corpus() {
    let dir = std::env::var("W3I_CORPUS").expect("W3I_CORPUS");
    let (mut n, mut bad) = (0, Vec::new());
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path().join("war3map.w3i");
        let Ok(bytes) = std::fs::read(&path) else { continue };
        n += 1;
        match parse(&bytes) {
            Ok(i) if i.to_bytes().unwrap() != bytes => bad.push(format!("{}: differs", path.display())),
            Ok(i) => {
                // And through the text form.
                let parsed = text::from_text(&text::to_text(&i));
                match parsed.info.map(|i| i.to_bytes().unwrap()) {
                    Some(b) if b == bytes => {}
                    _ => bad.push(format!("{}: differs through text: {:?}", path.display(), parsed.errors)),
                }
            }
            Err(e) => bad.push(format!("{}: {e}", path.display())),
        }
    }
    assert!(bad.is_empty(), "{} of {n}:\n{}", bad.len(), bad.join("\n"));
}

