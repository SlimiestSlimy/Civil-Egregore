//! A type registry for the tests: the mods a test writes, read from their
//! packages as a world's mods are.

use protocol::manifests::read_mods;
use protocol::mod_packages::{MANIFEST_FILE, PACKAGE_EXTENSION};
use protocol::type_registry::TypeRegistry;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use utilities::archive::write_archive;
use utilities::dynamic_library::library_file_name;

/// The registry of the mods `manifests` say -- each a manifest but its
/// hash, the hash of a stand-in library never loaded put in -- read from
/// packages written for the test `test_name`.
pub fn registry_of_mods(test_name: &str, manifests: &[&str]) -> TypeRegistry {
    let library_bytes = b"no code";
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join(test_name);
    std::fs::create_dir_all(&test_folder).expect("the test's folder");
    let package_paths: Vec<PathBuf> = manifests.iter().map(|manifest| {
        let (section_line, fields_and_types) = manifest.split_once('\n').expect("a mod's section, then its fields");
        let mod_name = section_line.trim_matches(':');
        let manifest = format!("{section_line}\n- hash : 0x{:x}\n{fields_and_types}", Sha256::digest(library_bytes));
        let library_file = library_file_name(mod_name);
        let package = write_archive(&[(MANIFEST_FILE, manifest.as_bytes()), (library_file.as_str(), library_bytes)]);
        let package_path = test_folder.join(format!("{mod_name}{PACKAGE_EXTENSION}"));
        std::fs::write(&package_path, package).expect("the package written");
        package_path
    }).collect();
    let registry = TypeRegistry::of_manifests(&read_mods(&package_paths).expect("read and checked"));
    _ = std::fs::remove_dir_all(&test_folder);
    registry
}

/// The registry of one common mod with a plane of each shape of
/// `plane_shapes` -- its width in bits, its tile's side in cells -- read
/// from a package written for the test `test_name`; and those planes'
/// type IDs, in that order.
pub fn registry_of_planes(test_name: &str, plane_shapes: &[(u32, u32)]) -> (TypeRegistry, Vec<u64>) {
    let mut manifest = ":planes:\n- side : common\n- version : 0\n".to_string();
    for (number, (width_bits, side_cells)) in plane_shapes.iter().enumerate() {
        manifest += &format!("\nplane_{number} :\n- kind : plane\n- width : {width_bits}\n- tile side : {side_cells}\n");
    }
    let registry = registry_of_mods(test_name, &[&manifest]);
    let plane_type_ids = (0..plane_shapes.len()).map(|number| registry.type_id_of(&format!("planes>plane_{number}")).expect("a plane registered")).collect();
    (registry, plane_type_ids)
}
