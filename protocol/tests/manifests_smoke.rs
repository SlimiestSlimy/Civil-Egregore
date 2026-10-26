//! A smoke test: five mods, every kind among them and a container
//! carrying two, read from their packages and checked together -- their
//! libraries hashed as their manifests say -- and what is wrong among
//! them refused.

use protocol::kinds::{Hook, Kind, Side};
use protocol::manifest_format::{read_mod_document, write_mod_document};
use protocol::manifests::read_mods;
use protocol::packages::{DEPENDENCIES_FOLDER, MANIFEST_FILE, PACKAGE_EXTENSION};
use protocol::type_names::TypeName;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use utilities::archive::write_archive;
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
fn named(mod_name: &str, type_name: &str) -> TypeName {
    TypeName { mod_name: mod_name.to_string(), type_name: type_name.to_string() }
}

/// Why `mods` and one mod more, `wrong`, are refused.
fn refusal_of(mods: &[PathBuf], wrong: PathBuf) -> String {
    read_mods(&[mods, &[wrong]].concat()).expect_err("refused").to_string()
}

/// A common mod, a server one on it, a common container carrying both,
/// a client and a server mod on the container, read as written, a mod
/// given twice kept once; each thing wrong with one mod more refused.
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
- tile size : 8

sheep_data :
- kind : attribute
- size : 2
", &[]);
    let base_server = package_of("base_server", Some(b"server bytes"), "\
:base_server:
- side : server
- version : 0.1
- hash : {hash}
- depends on :
    -- base_common : 0.1

sheep :
- kind : entity
- default attribute : base_common>sheep_data
- hooks :
    -- keeps the world hot

grass_grows :
- kind : sca proc
- planes :
    -- base_common>grass
- set up : grass_grows_set_up

world_generation :
- kind : worldgen
- set up : world_generation_set_up
- hooks :
    -- generates the world
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

grass_drawn :
- kind : render proc
- draw order : 3
- set up : grass_drawn_set_up

main_window :
- kind : gui
- set up : main_window_set_up
- hooks :
    -- shown by default
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
- set up : meadow_grows_set_up
"),
    ];
    let manifests = read_mods(&mods).expect("read and checked");
    let names: Vec<&str> = manifests.iter().map(|manifest| manifest.document().name.as_str()).collect();
    assert_eq!(names, ["base", "base_common", "base_server", "drawn", "meadow"]);
    let [container, common, server, drawn, meadow] = manifests.as_slice() else { panic!("five mods") };
    assert_eq!(server.package().to_string(), format!("{} > dependencies/base_server.zip", mods[0].display()));
    assert_eq!(common.document().types[0].kind(), &Kind::Plane { width_bits: 4, tile_size: 8 });
    assert_eq!((server.document().side, &server.document().depends_on[..]), (Side::Server, &[("base_common".to_string(), "0.1".to_string())][..]));
    assert_eq!(server.document().hash, Some(format!("0x{:x}", Sha256::digest(b"server bytes"))));
    let sheep = &server.document().types[0];
    assert_eq!((sheep.kind(), sheep.hooks()), (&Kind::Entity { default_attribute: Some(named("base_common", "sheep_data")) }, &[Hook::KeepsTheWorldHot][..]));
    assert_eq!((container.document().container, container.document().types.len()), (true, 0));
    assert_eq!((drawn.document().version.as_str(), drawn.document().types[0].kind(), drawn.document().types[1].hooks()), ("2.0 beta", &Kind::RenderProc { draw_order: 3 }, &[Hook::ShownByDefault][..]));
    let meadow_grows = &meadow.document().types[0];
    assert_eq!((meadow_grows.kind(), meadow_grows.set_up()), (&Kind::ScaProc { planes: vec![named("base_common", "grass")] }, Some("meadow_grows_set_up")));

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
- set up : grass_dies_set_up
";
    let refusal = refusal_of(&mods, mod_made(&test_folder, "out_of_reach", Some(b"bytes"), out_of_reach));
    assert!(refusal.contains("does not reach"), "{refusal}");
    let refusal = refusal_of(&mods, mod_made(&test_folder, "carrying", Some(b"bytes"), ":carrying:\n- side : common\n- version : 1\n- hash : {hash}\n- carried dependencies :\n    -- base_common : 0.1\n"));
    assert!(refusal.contains("carried dependencies"), "{refusal}");
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
    _ = std::fs::remove_dir_all(&test_folder);
}

/// Every kind's fields, every hook, both lists of dependencies and a
/// hash, read, written and read back: the same.
#[test]
fn every_field_written_reads_back() {
    let documents = [
        ":kinds_common:\n- side : common\n- version : 1.0 \"beta\"\n- hash : 0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n- depends on :\n    -- other : 2\n\nwater :\n- kind : plane\n- width : 8\n- tile size : 16\n\nhealth :\n- kind : attribute\n- size : 2\n\nask :\n- kind : client call\n- set up : ask_set_up\n",
        ":kinds_server:\n- side : server\n- version : 0.1\n- depends on :\n    -- kinds_common : 1.0\n- carried dependencies :\n    - carried_one : 3\n    -- carried_two : 4\n\nsheep :\n- kind : entity\n- default attribute : kinds_common>health\n- hooks :\n    -- keeps the world hot\n\nwolf :\n- kind : entity\n\nsheep_walks :\n- kind : entity proc\n- entity : sheep\n- set up : sheep_walks_set_up\n\nwater_flows :\n- kind : sca proc\n- planes :\n    - kinds_common>water\n    -- other>lava\n- set up : water_flows_set_up\n\nislands :\n- kind : worldgen\n- hooks :\n    -- generates the world\n- set up : islands_set_up\n",
        ":kinds_client:\n- side : client\n- version : 0.1\n\nwater_drawn :\n- kind : render proc\n- draw order : 3\n- set up : water_drawn_set_up\n\nmenu :\n- kind : gui\n- hooks :\n    -- shown by default\n- set up : menu_set_up\n\nsubmenu :\n- kind : gui\n- parent : menu\n- set up : submenu_set_up\n",
        ":kinds_container:\n- side : server\n- version : 0.1\n- container mod\n- carried dependencies :\n    -- kinds_server : 0.1\n",
    ];
    for text in documents {
        let read = read_mod_document(&cdnf::parse(text).expect("parsed")).expect("a mod document");
        let written = write_mod_document(&read);
        let read_back = read_mod_document(&cdnf::parse(&written).expect("written parses")).expect("written is a mod document");
        assert_eq!(read_back, read, "{written}");
    }
}
