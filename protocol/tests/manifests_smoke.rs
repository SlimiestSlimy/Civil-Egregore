//! A smoke test: five mods, every kind among them and a container
//! carrying two, read from their packages and checked together -- their
//! libraries hashed as their manifests say -- and what is wrong among
//! them refused.

use protocol::type_kinds::{Kind, PlaneWidth, Side, TileSide};
use protocol::manifest_format::{read_mod_document, write_mod_document, DeclaredDependency};
use protocol::manifests::read_mods;
use protocol::mod_packages::{DEPENDENCIES_FOLDER, MANIFEST_FILE, PACKAGE_EXTENSION};
use protocol::simulation_time::{SimulationSeconds, TICKS_PER_SIMULATION_SECOND};
use protocol::type_names::QualifiedTypeName;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use utilities::archive::write_archive;
use utilities::chance::{Chance, PARTS};
use utilities::dynamic_library::library_file_name;

/// The package of the mod `mod_name`: its manifest, `{hash}` in it the
/// library's hash; its library, holding `library_bytes`, if it has one;
/// and the packages `carried`, each by its mod's name.
fn package_of(mod_name: &str, library_bytes: Option<&[u8]>, manifest: &str, carried: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let manifest = library_bytes.map_or(manifest.to_string(), |library_bytes| manifest.replace("{hash}", &format!("0x{:x}", Sha256::digest(library_bytes))));
    let library_file = library_file_name(mod_name);
    let carried_paths: Vec<String> = carried.iter().map(|(carried_name, _)| format!("{DEPENDENCIES_FOLDER}{carried_name}{PACKAGE_EXTENSION}")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![(MANIFEST_FILE, manifest.as_bytes())];
    entries.extend(library_bytes.map(|library_bytes| (library_file.as_str(), library_bytes)));
    entries.extend(carried_paths.iter().zip(carried).map(|(path, (_, package))| (path.as_str(), package.as_slice())));
    write_archive(&entries)
}

/// The package `package` written in `test_folder` as `file_name`: where.
fn written(test_folder: &Path, file_name: &str, package: &[u8]) -> PathBuf {
    std::fs::create_dir_all(test_folder).expect("the test's folder");
    let path = test_folder.join(format!("{file_name}{PACKAGE_EXTENSION}"));
    std::fs::write(&path, package).expect("the package written");
    path
}

/// The package of the mod `mod_name`, carrying none, written in
/// `test_folder`: where.
fn mod_made(test_folder: &Path, mod_name: &str, library_bytes: Option<&[u8]>, manifest: &str) -> PathBuf {
    written(test_folder, mod_name, &package_of(mod_name, library_bytes, manifest, &[]))
}

/// The type `type_name` of the mod `mod_name`.
fn named(mod_name: &str, type_name: &str) -> QualifiedTypeName {
    QualifiedTypeName { mod_name: mod_name.to_string(), type_name: type_name.to_string() }
}

/// Why `mods` and one mod more, `wrong`, are refused.
fn refusal_of(mods: &[PathBuf], wrong: PathBuf) -> String {
    read_mods(&[mods, &[wrong]].concat()).expect_err("refused").to_string()
}

/// A common mod, a server one on it, a common container carrying both,
/// a client and a server mod on the container, read as written, a mod
/// given twice kept once; each thing wrong with one mod more -- or two,
/// for a ring -- refused.
#[test]
fn five_mods_read_as_written() {
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("manifests_smoke");
    _ = std::fs::remove_dir_all(&test_folder);
    let base_common = package_of("base_common", Some(b"common bytes"), "\
:base_common:
- side : common
- version : 0.1
- hash : {hash}

grass :
- kind : plane
- width : 4
- tile side : 8
", &[]);
    let base_server = package_of("base_server", Some(b"server bytes"), "\
:base_server:
- side : server
- version : 0.1
- hash : {hash}
- depends on :
    -- base_common : 0.1

grass_grows :
- kind : sca proc
- planes :
    -- base_common>grass
- seconds per sample : 1
- set up : GRASS_GROWS_SET_UP
", &[]);
    let base = package_of("base", None, "\
:base:
- side : common
- version : 0.1
- container mod
- depends on :
    - base_common : 0.1
    -- base_server : 0.1
", &[("base_common", base_common.clone()), ("base_server", base_server)]);
    let mods = [
        written(&test_folder, "base", &base),
        mod_made(&test_folder, "drawn", Some(b"client bytes"), "\
:drawn:
- side : client
- version : 2.0 beta
- hash : {hash}
- depends on :
    -- base : 0.1
"),
        mod_made(&test_folder, "meadow", Some(b"meadow bytes"), "\
:meadow:
- side : server
- version : 1
- hash : {hash}
- depends on :
    -- base : 0.1

meadow_grows :
- kind : sca proc
- planes :
    -- base_common>grass
- seconds per sample : 0.25
- set up : MEADOW_GROWS_SET_UP
"),
    ];
    let manifests = read_mods(&mods).expect("read and checked");
    let names: Vec<&str> = manifests.iter().map(|manifest| manifest.document().name.as_str()).collect();
    assert_eq!(names, ["base", "base_common", "base_server", "drawn", "meadow"]);
    let [container, common, server, drawn, meadow] = manifests.as_slice() else { panic!("five mods") };
    assert_eq!(server.package().to_string(), format!("{} > dependencies/base_server.zip", mods[0].display()));
    assert_eq!(common.document().types[0].kind(), &Kind::Plane { width: PlaneWidth::Bits4, tile_side: TileSide::Cells8 });
    assert_eq!((server.document().side, &server.document().depends_on[..]), (Side::Server, &[DeclaredDependency { mod_name: "base_common".to_string(), version: "0.1".to_string(), source: None }][..]));
    assert_eq!(server.document().hash, Some(format!("0x{:x}", Sha256::digest(b"server bytes"))));
    let grass_grows = &server.document().types[0];
    assert_eq!((grass_grows.kind(), grass_grows.set_up_static_name()), (&Kind::ScaProc { planes: vec![named("base_common", "grass")], mean_simulation_seconds_per_sample: SimulationSeconds::read("1").expect("a span") }, Some("GRASS_GROWS_SET_UP")));
    assert_eq!((container.document().is_container, container.document().types.len()), (true, 0));
    assert_eq!((drawn.document().side, drawn.document().version.as_str()), (Side::Client, "2.0 beta"));
    let meadow_grows = &meadow.document().types[0];
    assert_eq!((meadow_grows.kind(), meadow_grows.set_up_static_name()), (&Kind::ScaProc { planes: vec![named("base_common", "grass")], mean_simulation_seconds_per_sample: SimulationSeconds::read("0.25").expect("a span") }, Some("MEADOW_GROWS_SET_UP")));

    let refusal = refusal_of(&mods, mod_made(&test_folder, "old_version", Some(b"bytes"), ":old_version:\n- side : common\n- version : 1\n- hash : {hash}\n- depends on :\n    -- base_common : 0.2\n"));
    assert!(refusal.contains("base_common is 0.1"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "wrong_side", Some(b"bytes"), ":wrong_side:\n- side : client\n- version : 1\n- hash : {hash}\n- depends on :\n    -- base_server : 0.1\n"));
    assert!(refusal.contains("depends on base_server"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "server_pack", None, ":server_pack:\n- side : server\n- version : 1\n- container mod\n- depends on :\n    -- drawn : 2.0 beta\n"));
    assert!(refusal.contains("depends on drawn"), "{refusal}");
    let out_of_reach = "\
:out_of_reach:
- side : server
- version : 1
- hash : {hash}

grass_dies :
- kind : sca proc
- planes :
    -- base_common>grass
- seconds per sample : 1
- set up : GRASS_DIES_SET_UP
";
    let refusal = refusal_of(&mods, mod_made(&test_folder, "out_of_reach", Some(b"bytes"), out_of_reach));
    assert!(refusal.contains("does not reach"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "carrying", Some(b"bytes"), ":carrying:\n- side : common\n- version : 1\n- hash : {hash}\n- carried dependencies :\n    -- base_common : 0.1\n"));
    assert!(refusal.contains("carried dependencies"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "sourced", Some(b"bytes"), ":sourced:\n- side : common\n- version : 1\n- hash : {hash}\n- depends on :\n    -- base_common :\n        - version : 0.1\n        -- source : ../base_common\n"));
    assert!(refusal.contains("source"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "protocol", Some(b"bytes"), ":protocol:\n- side : common\n- version : 1\n- hash : {hash}\n"));
    assert!(refusal.contains("no name"), "{refusal}");
    let given_twice = read_mods(&[&mods[..], &[written(&test_folder, "base_common_again", &base_common)]].concat()).expect("the same mod twice");
    assert_eq!(given_twice, manifests, "the same mod twice kept once");

    let other_common = package_of("base_common", Some(b"other bytes"), ":base_common:\n- side : common\n- version : 0.1\n- hash : {hash}\n", &[]);
    let refusal = refusal_of(&mods, written(&test_folder, "other_common", &other_common));
    assert!(refusal.contains("its manifest not the same"), "{refusal}");
    let stray = write_archive(&[(MANIFEST_FILE, b":stray:\n- side : common\n- version : 1\n- container mod\n"), ("notes.txt", b"")]);
    let refusal = refusal_of(&mods, written(&test_folder, "stray", &stray));
    assert!(refusal.contains("notes.txt: neither"), "{refusal}");
    let no_library = format!(":no_library:\n- side : common\n- version : 1\n- hash : 0x{}\n", "0".repeat(64));
    let refusal = refusal_of(&mods, mod_made(&test_folder, "no_library", None, &no_library));
    assert!(refusal.contains("no library"), "{refusal}");
    let wrong_hash = format!(":wrong_hash:\n- side : common\n- version : 1\n- hash : 0x{}\n", "0".repeat(64));
    let refusal = refusal_of(&mods, mod_made(&test_folder, "wrong_hash", Some(b"bytes"), &wrong_hash));
    assert!(refusal.contains("its hash is"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "lonely", Some(b"bytes"), ":lonely:\n- side : common\n- version : 1\n- hash : {hash}\n- depends on :\n    -- nowhere : 1\n"));
    assert!(refusal.contains("depends on nowhere: no such mod"), "{refusal}");
    let ring_one = mod_made(&test_folder, "ring_one", Some(b"bytes"), ":ring_one:\n- side : common\n- version : 1\n- hash : {hash}\n- depends on :\n    -- ring_two : 1\n");
    let ring_two = mod_made(&test_folder, "ring_two", Some(b"bytes"), ":ring_two:\n- side : common\n- version : 1\n- hash : {hash}\n- depends on :\n    -- ring_one : 1\n");
    let refusal = read_mods(&[&mods[..], &[ring_one, ring_two]].concat()).expect_err("a ring").to_string();
    assert!(refusal.contains("depends on itself, through the mods it depends on"), "{refusal}");
    // A sca proc of the server, depending on the container, its planes written in.
    let sampling = |planes: &str| format!(":sampling:\n- side : server\n- version : 1\n- hash : {{hash}}\n- depends on :\n    -- base : 0.1\n\ngrass_dies :\n- kind : sca proc\n- planes :\n{planes}- seconds per sample : 1\n- set up : GRASS_DIES_SET_UP\n");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "sampling", Some(b"bytes"), &sampling("    -- base_server>grass_grows\n")));
    assert!(refusal.contains("planes: base_server>grass_grows is no plane"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "sampling", Some(b"bytes"), &sampling("    -- base_common>clover\n")));
    assert!(refusal.contains("planes: no type is base_common>clover"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "sampling", Some(b"bytes"), &sampling("    - base_common>grass\n    -- base_common>grass\n")));
    assert!(refusal.contains("planes: base_common>grass said twice"), "{refusal}");
    let twice = ":twice:\n- side : common\n- version : 1\n- hash : {hash}\n\nwater :\n- kind : plane\n- width : 1\n- tile side : 1\n\nwater :\n- kind : plane\n- width : 8\n- tile side : 1\n";
    let refusal = refusal_of(&mods, mod_made(&test_folder, "twice", Some(b"bytes"), twice));
    assert!(refusal.contains("a type named twice: water"), "{refusal}");
    let off_side = ":off_side:\n- side : common\n- version : 1\n- hash : {hash}\n\nflows :\n- kind : sca proc\n- planes :\n    -- base_common>grass\n- seconds per sample : 1\n- set up : FLOWS_SET_UP\n";
    let refusal = refusal_of(&mods, mod_made(&test_folder, "off_side", Some(b"bytes"), off_side));
    assert!(refusal.contains("flows: a sca proc has no place in a mod of the common side"), "{refusal}");
    let coded_plane = ":coded_plane:\n- side : common\n- version : 1\n- hash : {hash}\n\nwater :\n- kind : plane\n- width : 1\n- tile side : 1\n- set up : WATER_SET_UP\n";
    let refusal = refusal_of(&mods, mod_made(&test_folder, "coded_plane", Some(b"bytes"), coded_plane));
    assert!(refusal.contains("set up: a plane has no code to make"), "{refusal}");
    _ = std::fs::remove_dir_all(&test_folder);
}

/// Every kind's fields, both lists of dependencies and a hash, read,
/// written and read back: the same.
#[test]
fn every_field_written_reads_back() {
    let documents = [
        ":kinds_common:\n- side : common\n- version : 1.0 \"beta\"\n- hash : 0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n- depends on :\n    -- other : 2\n\nwater :\n- kind : plane\n- width : 8\n- tile side : 16\n",
        ":kinds_server:\n- side : server\n- version : 0.1\n- depends on :\n    -- kinds_common : 1.0\n- carried dependencies :\n    - carried_one :\n        - version : 3\n        -- source : ../carried_one\n    -- carried_two : 4\n\nwater_flows :\n- kind : sca proc\n- planes :\n    - kinds_common>water\n    -- other>lava\n- seconds per sample : 12.5\n- set up : WATER_FLOWS_SET_UP\n",
        ":kinds_client:\n- side : client\n- version : 0.1\n",
        ":kinds_container:\n- side : server\n- version : 0.1\n- container mod\n- carried dependencies :\n    -- kinds_server : 0.1\n",
    ];
    for text in documents {
        let read = read_mod_document(&cdnf::parse(text).expect("parsed")).expect("a mod document");
        let written = write_mod_document(&read).expect("its name a section's");
        let read_back = read_mod_document(&cdnf::parse(&written).expect("written parses")).expect("written is a mod document");
        assert_eq!(read_back, read, "{written}");
    }
}

/// A sca proc's seconds per sample: read exactly, written back as read,
/// made a chance a tick to the nearest part -- and refused unless more
/// than 0 and digits with a point if wanted.
#[test]
fn seconds_per_sample_read_exactly() {
    let seconds = |text: &str| SimulationSeconds::read(text);
    assert_eq!(seconds("1.50"), seconds("1.5"));
    assert_eq!(["0.25", "1.50", "12", "007.010"].map(|text| seconds(text).expect("a span").to_string()), ["0.25", "1.5", "12", "7.01"]);
    assert_eq!(seconds("1").expect("a span").chance_per_tick(), Chance::of_parts(PARTS / TICKS_PER_SIMULATION_SECOND));
    assert_eq!(seconds("0.5").expect("a span").chance_per_tick(), Chance::of_parts(2 * PARTS / TICKS_PER_SIMULATION_SECOND));
    assert_eq!(seconds("3").expect("a span").chance_per_tick(), Chance::of_parts((PARTS + 3 * TICKS_PER_SIMULATION_SECOND / 2) / (3 * TICKS_PER_SIMULATION_SECOND)));
    assert!(seconds("0.001").expect("a span").chance_per_tick().is_always(), "shorter than a tick");
    for refused in ["0", "0.000", ".5", "abc", "-1", "1e3", "", "0.0000000000000000001"] {
        assert_eq!(seconds(refused), None, "`{refused}`");
    }
    let sca_proc = |seconds_field: &str| read_mod_document(&cdnf::parse(&format!(":rated:\n- side : server\n- version : 1\n\nflows :\n- kind : sca proc\n- planes :\n    -- water\n{seconds_field}- set up : FLOWS_SET_UP\n")).expect("parsed"));
    assert!(sca_proc("- seconds per sample : 2\n").is_ok());
    for refused in ["", "- seconds per sample : 0\n", "- seconds per sample : soon\n"] {
        let refusal = sca_proc(refused).expect_err("refused").what;
        assert!(refusal.contains("seconds per sample"), "{refusal}");
    }
}
