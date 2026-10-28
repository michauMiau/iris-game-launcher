use crate::*;

#[test]
fn platform_shim_step_places_the_dll_and_never_twice() {
    let root = std::env::temp_dir().join(format!("iris-shim-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let placed = platform_shim_step(&root).expect("shim written into the game folder");
    assert_eq!(placed, root.join("LibOVRPlatform64_1.dll"));
    assert!(placed.is_file());
    assert!(
        platform_shim_step(&root).is_none(),
        "existing shim must not be rewritten"
    );
    assert!(platform_shim_step(&root.join("nope")).is_none());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn platform_shim_step_replaces_a_stale_shim() {
    // Launcher upgrades must refresh the DLL inside game folders: an exists()
    // check once kept pre-timeline shims running after package updates.
    let root = std::env::temp_dir().join(format!("iris-shim-stale-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("LibOVRPlatform64_1.dll"), b"old-shim-bytes").unwrap();
    let placed = platform_shim_step(&root).expect("stale shim must be replaced");
    assert_eq!(placed, root.join("LibOVRPlatform64_1.dll"));
    assert!(placed.metadata().unwrap().len() > 15);
    assert!(
        platform_shim_step(&root).is_none(),
        "fresh shim must not be rewritten"
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn embedded_logo_texture_uses_its_own_dimensions() {
    // The transparent rebuild shipped a non-square crop; the header and the
    // window icon must take the asset's real size, never a hardcoded square.
    let img = image::load_from_memory(include_bytes!("../../../assets/iris-logo.png")).unwrap();
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba.into_raw());
    assert_eq!(ci.pixels.len(), w * h);
}

#[test]
fn embedded_shim_is_the_asset_file_and_carries_unlock_export() {
    let embedded = include_bytes!("../../../assets/platform-shim.dll");
    let on_disk = std::fs::read(std::path::Path::new("../assets/platform-shim.dll")).unwrap();
    assert_eq!(
        embedded.len(),
        on_disk.len(),
        "embedded bytes must match the asset file"
    );
    assert_eq!(
        embedded,
        on_disk.as_slice(),
        "stale embed: rebuild shim and copy to assets"
    );
    // The achievement-id capture is a newer export arm; an embed older than
    // the repo file would still byte-match, so check the name is present.
    let name = b"ovr_Achievements_Unlock";
    assert!(embedded.windows(name.len()).any(|w| w == name));
}
