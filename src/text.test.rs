use super::*;
use crate::tests::full;

fn back(text: &str) -> Info {
    let p = from_text(text);
    assert_eq!(p.errors, vec![], "{text}");
    p.info.unwrap()
}

fn errors(text: &str) -> Vec<String> {
    let p = from_text(text);
    assert!(p.info.is_none(), "{text}");
    p.errors.into_iter().map(|e| format!("{}: {}", e.line, e.message)).collect()
}

#[test]
fn text_reads_back_to_the_same_bytes() {
    for v in VERSIONS {
        let i = full(v);
        let text = to_text(&i);
        let b = back(&text);
        assert_eq!(b.to_bytes().unwrap(), i.to_bytes().unwrap(), "{text}");
        assert!(is_info_text(&bni_rs::parse(&text)));
        assert_eq!(text.contains("script="), v >= 28);
        assert_eq!(text.contains("[item_table]"), v >= 25);
        assert_eq!(text.contains("prologue_screen="), v == 18);
    }
    let text = to_text(&full(31));
    for line in [
        "version=31 // 1.32 and later\n",
        "description=\"Line one\"\\n\"Line two // not a comment\"\n",
        "author=\"Me, \"\\u{22}\"quoted\"\\u{22}\n",
        "flags=0x80008C14 // melee map, masked areas are partially visible, map properties menu opened at least once, show water waves on cliff shores, use item classification system, other bits 0x80000000\n",
        "tileset=L // Lordaeron Summer\n",
        "weather=RAhr\n",
        "light_tileset=A // Ashenvale\n",
        "start_y=0x7FC00001\n",
        "[force]\nflags=0x0000000B // allied, allied victory, shared vision\nplayers=0x00000003 // 1, 2\n",
        "columns=0,2 // unit, item\nrow=50,hfoo,0x00000000\n",
        "set=10,ratf,90,rde1\nset=\n",
    ] {
        assert!(text.contains(line), "{line:?} in\n{text}");
    }
}

#[test]
fn left_out_fields_take_the_defaults() {
    let i = back("[w3i]\nversion=18\nname=\"A\"\n[player]\nname=\"P\"\n");
    assert_eq!(i.version, 18);
    assert_eq!((i.name.as_str(), i.loading_screen, i.prologue_screen), ("A", -1, -1));
    assert_eq!(i.players[0].name, "P");
}

#[test]
fn cut_files() {
    let mut i = full(25);
    i.upgrades.clear();
    i.techs.clear();
    i.unit_tables.clear();
    i.item_tables.clear();
    i.cut = Some(Cut { after: Part::Forces, bytes: vec![255] });
    let bytes = i.to_bytes().unwrap();
    assert_eq!(crate::parse(&bytes).unwrap().to_bytes().unwrap(), bytes);
    let text = to_text(&i);
    assert!(text.contains("cut=forces // "), "{text}");
    assert!(text.contains("cut_bytes=255\n"), "{text}");
    assert_eq!(back(&text).to_bytes().unwrap(), bytes);
    let e = errors(&format!("{text}\n[tech]\nid=hfoo\n"));
    assert!(e[0].contains("cut after its forces"), "{e:?}");
}

#[test]
fn errors_in_the_text() {
    let e = errors(
        "[w3i]\nversion=25\nprologue_screen=1\nscript=1\ncamera_complements=1,2\nflags=x\nname=a,b\nfoo=1\n[player]\nenemy_low=1\n[unit_table]\ncolumns=0\nrow=1,hfoo,hfoo\n[what]\n",
    );
    let want = [
        "3: `prologue_screen` is only in version 18",
        "4: `script` is in version 28 (1.31) and later; this file is 25",
        "5: `camera_complements` has 4 values",
        "6: `flags`: a whole number",
        "7: `name` is one string",
        "8: no field `foo` in [w3i]",
        "10: `enemy_low` is in version 31",
        "13: a row is a chance, then an id per column: 2 ids for 1 columns",
        "14: no section [what]",
    ];
    assert_eq!(e.len(), want.len(), "{e:#?}");
    for (got, want) in e.iter().zip(want) {
        assert!(got.starts_with(want), "{got} / {want}");
    }
    assert!(errors("[w3i]\nversion=18\n[item_table]\n")[0].contains("version 25"));
    assert!(errors("[w3i]\nversion=24\n")[0].contains("the version is 18"));
    assert!(errors("[cameras]\n")[0].contains("starts with [w3i]"));
}
