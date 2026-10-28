//! A type registry for the tests: one mod of the planes a test asks for,
//! read from its package as a world's mods are.

use protocol::manifests::read_mods;
use protocol::mod_packages::{MANIFEST_FILE, PACKAGE_EXTENSION};
use protocol::type_registry::TypeRegistry;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use utilities::archive::write_archive;
use utilities::dynamic_library::library_file_name;

/// The registry of one common mod with a plane of each shape of
/// `plane_shapes` -- its width in bits, its tile's side in cells -- read
/// from a package written for the test `test_name`; and those planes'
/// type IDs, in that order.
pub fn registry_of_planes(test_name: &str, plane_shapes: &[(u32, u32)]) -> (TypeRegistry, Vec<u64>) {
    let library_bytes = b"no code";
    let mut manifest = format!(":planes:\n- side : common\n- version : 0\n- hash : 0x{:x}\n", Sha256::digest(library_bytes));
    for (number, (width_bits, side_cells)) in plane_shapes.iter().enumerate() {
        manifest += &format!("\nplane_{number} :\n- kind : plane\n- width : {width_bits}\n- tile side : {side_cells}\n");
    }
    let library_file = library_file_name("planes");
    let package = write_archive(&[(MANIFEST_FILE, manifest.as_bytes()), (library_file.as_str(), library_bytes)]);
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join(test_name);
    std::fs::create_dir_all(&test_folder).expect("the test's folder");
    let package_path = test_folder.join(format!("planes{PACKAGE_EXTENSION}"));
    std::fs::write(&package_path, package).expect("the package written");
    let registry = TypeRegistry::of_manifests(&read_mods(&[package_path]).expect("read and checked"));
    _ = std::fs::remove_dir_all(&test_folder);
    let plane_type_ids = (0..plane_shapes.len()).map(|number| {
        let qualified_name = format!("planes>plane_{number}");
        registry.types().iter().find(|registered| registered.qualified_name() == qualified_name).expect("a plane registered").type_id()
    }).collect();
    (registry, plane_type_ids)
}
