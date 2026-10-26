//! A smoke test: two mods, every kind among them, read and checked
//! together -- their libraries files of their own, hashed as their
//! manifests say.

use protocol::kinds::{Hook, Kind, Side};
use protocol::manifests::{read_mods, MANIFEST_FILE};
use utilities::hash::hash_bytes;
use protocol::type_names::{BundleName, TypeName};
use std::path::{Path, PathBuf};

/// A mod's folder made in the test's own, its libraries -- each its
/// name and the bytes it holds -- written, and its manifest: `manifest`,
/// each `{name}` in it the hash of the library of that name.
fn mod_made(test_folder: &Path, mod_name: &str, libraries: &[(&str, &[u8])], manifest: &str) -> PathBuf {
    let mod_folder = test_folder.join(mod_name);
    std::fs::create_dir_all(&mod_folder).expect("the mod's folder");
    let mut manifest = manifest.to_string();
    for &(library_name, library_bytes) in libraries {
        std::fs::write(mod_folder.join(library_name), library_bytes).expect("a library written");
        manifest = manifest.replace(&format!("{{{library_name}}}"), &format!("{:016x}", hash_bytes(library_bytes)));
    }
    std::fs::write(mod_folder.join(MANIFEST_FILE), manifest).expect("the manifest written");
    mod_folder
}

/// The bundle `bundle` of the mod `mod_name`.
fn bundle_named(mod_name: &str, bundle: &str) -> BundleName {
    BundleName { mod_name: mod_name.to_string(), bundle: bundle.to_string() }
}

/// The type `type_name` of the bundle `bundle` of the mod `mod_name`.
fn named(mod_name: &str, bundle: &str, type_name: &str) -> TypeName {
    TypeName { bundle_name: bundle_named(mod_name, bundle), type_name: type_name.to_string() }
}

/// Two mods, one depending on the other's common bundle, read as
/// written; a client bundle depending on a server one, refused.
#[test]
fn two_mods_read_as_written() {
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("manifests_smoke");
    _ = std::fs::remove_dir_all(&test_folder);
    let base = mod_made(&test_folder, "base", &[("libbase_common.so", b"common bytes"), ("libbase_server.so", b"server bytes")], "\
:base:

::base_common::
- side : common
- library : libbase_common.so
- hash : {libbase_common.so}

grass :
- kind : plane
- width : 4
- tile size : 8

sheep_data :
- kind : attribute
- size : 2

::base_server::
- side : server
- library : libbase_server.so
- hash : {libbase_server.so}
- depends on :
    -- base_common

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
");
    let drawn = mod_made(&test_folder, "drawn", &[("libdrawn.so", b"client bytes")], "\
:drawn:

::drawn_client::
- side : client
- library : libdrawn.so
- hash : {libdrawn.so}
- depends on :
    -- base>base_common

grass_drawn :
- kind : render proc
- draw order : 3
- set up : grass_drawn_set_up

main_window :
- kind : gui
- set up : main_window_set_up
- hooks :
    -- shown by default

");
    let manifests = read_mods(&[base.clone(), drawn.clone()]).expect("read and checked");
    let [base_manifest, drawn_manifest] = manifests.as_slice() else { panic!("two mods") };
    assert_eq!((base_manifest.name(), drawn_manifest.name()), ("base", "drawn"));
    let server = &base_manifest.bundles()[1];
    assert_eq!((server.name(), server.fields().side, &server.fields().depends_on[..]), ("base_server", Side::Server, &[bundle_named("base", "base_common")][..]));
    assert_eq!(server.fields().library_hash, hash_bytes(b"server bytes"));
    let sheep = &server.types()[0];
    assert_eq!((sheep.kind(), sheep.hooks()), (&Kind::Entity { default_attribute: Some(named("base", "base_common", "sheep_data")) }, &[Hook::KeepsTheWorldHot][..]));
    assert_eq!(base_manifest.bundles()[0].types()[0].kind(), &Kind::Plane { width_bits: 4, tile_size: 8 });
    let grass_grows = &server.types()[1];
    assert_eq!((grass_grows.kind(), grass_grows.set_up()), (&Kind::ScaProc { planes: vec![named("base", "base_common", "grass")] }, Some("grass_grows_set_up")));
    let client = &drawn_manifest.bundles()[0];
    assert_eq!((client.fields().side, client.types()[0].kind(), client.types()[1].hooks()), (Side::Client, &Kind::RenderProc { draw_order: 3 }, &[Hook::ShownByDefault][..]));

    let wrong_side = mod_made(&test_folder, "wrong_side", &[("libwrong.so", b"wrong bytes")], "\
:wrong_side:

::wrong_client::
- side : client
- library : libwrong.so
- hash : {libwrong.so}
- depends on :
    -- base>base_server
");
    let refusal = read_mods(&[base, drawn, wrong_side]).expect_err("a client bundle on a server one");
    assert!(refusal.what.contains("depends on base>base_server"), "{refusal}");
    _ = std::fs::remove_dir_all(&test_folder);
}
