use crate::fetcher::*;
use std::path::PathBuf;
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("meta-pcvr-{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
#[test]
fn intact_files_are_reused_and_partial_ones_are_not() {
    let dir = temp_dir("reuse");
    let path = dir.join("asset.bin");
    std::fs::write(&path, b"hello world").unwrap();
    let sha = |bytes: &[u8]| {
        let mut h = Sha256::new();
        h.update(bytes);
        format!("{:x}", h.finalize())
    };
    let good = sha(b"hello world");
    assert_eq!(file_sha256(&path).unwrap(), good);
    // Stored hash + actual hash + size must ALL agree to skip a download.
    assert!(reusable(Some(&good), Some(&good), true));
    assert!(!reusable(Some(&good), Some(&good), false)); // truncated file
    assert!(!reusable(Some(&good), Some(&sha(b"corrupt")), true)); // tampered
    assert!(!reusable(None, Some(&good), true)); // no saved manifest hashes
    assert!(!reusable(Some(&good), None, true)); // unreadable file
    assert!(!reusable(None, None, true));
}
#[test]
fn manifest_keys_become_real_folders_not_flat_names() {
    // Measured bug: Bonelab keys are `Data\app.info`; replacing only '/'
    // produced one flat file literally named with backslashes.
    let sep = std::path::MAIN_SEPARATOR_STR;
    let rel = normalize_manifest_key("BONELAB_Oculus_Windows64_Data\\app.info").unwrap();
    assert_eq!(rel, format!("BONELAB_Oculus_Windows64_Data{sep}app.info"));
    assert_eq!(rel.split(sep).count(), 2);
    // Mixed and leading separators normalize too.
    let rel = normalize_manifest_key("/Data/Plugins\\x.dll").unwrap();
    assert_eq!(rel, format!("Data{sep}Plugins{sep}x.dll"));
    // Traversal must never write outside the install folder.
    assert!(normalize_manifest_key("..\\evil.dll").is_err());
    assert!(normalize_manifest_key("Data/../../evil.dll").is_err());
    assert!(normalize_manifest_key("\\\\").is_err());
    // A key with no separator stays a plain file name.
    assert_eq!(
        normalize_manifest_key("baselib.dll").unwrap(),
        "baselib.dll"
    );
}
#[test]
fn verify_files_passes_and_fails() {
    let dir = temp_dir("verify");
    std::fs::write(dir.join("a.txt"), b"hello").unwrap();
    std::fs::write(dir.join("b.txt"), b"world").unwrap();
    let sha = |bytes: &[u8]| {
        let mut h = Sha256::new();
        h.update(bytes);
        format!("{:x}", h.finalize())
    };
    let hashes = serde_json::json!({
        "a.txt": {"sha256": sha(b"hello"), "size": 5},
        "b.txt": {"sha256": sha(b"world"), "size": 5},
    });
    std::fs::write(
        dir.join(".oculus-hashes.json"),
        serde_json::to_vec(&hashes).unwrap(),
    )
    .unwrap();
    assert_eq!(verify_files(&dir).unwrap(), (2, 0, 0));
    std::fs::write(dir.join("b.txt"), b"tampered").unwrap();
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    assert_eq!(verify_files(&dir).unwrap(), (0, 1, 1));
}
/// The MelonLoader installer unpacks zip assets; this proves nested paths
/// survive and that a broken archive is rejected, not silently ignored.
#[test]
fn unpack_zip_keeps_nested_paths_and_rejects_garbage() {
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;
    let opts = FileOptions::default().compression_method(CompressionMethod::Stored);
    let mut w = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    w.start_file("MelonLoader/version.dll", opts).unwrap();
    std::io::Write::write_all(&mut w, b"dummy").unwrap();
    w.start_file("data/a.txt", opts).unwrap();
    std::io::Write::write_all(&mut w, b"nested").unwrap();
    let bytes = w.finish().unwrap().into_inner();
    let dir = temp_dir("melon-unpack");
    unpack_zip(bytes, &dir).unwrap();
    assert_eq!(
        std::fs::read(dir.join("MelonLoader/version.dll")).unwrap(),
        b"dummy"
    );
    assert_eq!(std::fs::read(dir.join("data/a.txt")).unwrap(), b"nested");
    assert!(!dir.join("nope.txt").exists());
    assert!(unpack_zip(vec![1, 2, 3], &dir).is_err());
}
/// Proton GE ships as tar.gz; this proves the tool layout survives and a
/// corrupt archive is rejected rather than half-extracted.
#[cfg(not(target_os = "windows"))]
#[test]
fn unpack_tar_keeps_layout_and_rejects_garbage() {
    let src = temp_dir("proton-src");
    std::fs::create_dir_all(src.join("proton/bin")).unwrap();
    std::fs::write(src.join("proton/proton"), b"#proton script").unwrap();
    std::fs::write(src.join("proton/bin/wine"), b"wine binary").unwrap();
    let archive = temp_dir("proton-archive").join("asset.tar.gz");
    let ok = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&src)
        .arg(".")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(ok, "tar create failed");
    let bytes = std::fs::read(&archive).unwrap();
    let dest = temp_dir("proton-dest");
    unpack_tar(&bytes, &dest).unwrap();
    assert_eq!(
        std::fs::read(dest.join("proton/proton")).unwrap(),
        b"#proton script"
    );
    assert_eq!(
        std::fs::read(dest.join("proton/bin/wine")).unwrap(),
        b"wine binary"
    );
    assert!(!dest.join("absent.txt").exists());
    assert!(unpack_tar(b"not a tar archive", &dest).is_err());
}
#[test]
fn segment_shorter_than_header_rejects_and_valid_segment_decodes() {
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use sha2::Digest;
    use std::io::Write;
    let mut payload = Vec::new();
    let mut encoder = DeflateEncoder::new(&mut payload, Compression::default());
    encoder.write_all(b"hello world").unwrap();
    encoder.finish().unwrap();
    let mut hasher = sha2::Sha256::new();
    hasher.update(&payload);
    let sha = format!("{:x}", hasher.finalize());
    let mut blob = vec![0xAA, 0xBB];
    blob.extend_from_slice(&payload);
    assert_eq!(
        decode_segment(&blob, &sha, "game.exe").unwrap(),
        b"hello world"
    );
    // Short wire response must be an error, not a panic on slicing.
    assert!(decode_segment(&blob[..1], &sha, "game.exe").is_err());
    // Wrong sha rejects too.
    assert!(decode_segment(&blob, "deadbeef", "game.exe").is_err());
    // Bonelab-style manifests hash the DECOMPRESSED chunk instead of the
    // deflate payload (measured 2026-10-24). That rule must also pass.
    let mut h = sha2::Sha256::new();
    h.update(b"hello world");
    let chunk_sha = format!("{:x}", h.finalize());
    assert_eq!(
        decode_segment(&blob, &chunk_sha, "game.exe").unwrap(),
        b"hello world"
    );
}
/// Live GitHub downloader smoke (public repos, no secrets). Run manually:
/// `cargo test --release -- --ignored github_downloader`
#[test]
#[ignore]
fn github_downloader_end_to_end() {
    for repo in ["GloriousEggroll/proton-ge-custom", "LavaGang/MelonLoader"] {
        let avoid: &[&str] = if repo == "LavaGang/MelonLoader" {
            &["linux", "mac", "macos"]
        } else {
            &["win", "mac"]
        };
        let rel = github_releases(repo, avoid).expect("github_releases failed");
        assert!(!rel.is_empty(), "{repo}: no releases listed");
        let (_tag, name, url) = rel[0].clone();
        let low = name.to_lowercase();
        for frag in avoid {
            assert!(
                !low.contains(frag),
                "{repo}: picked a {frag} build ({name}) — wrong platform"
            );
        }
        let dest = temp_dir(&format!("gh-{}", name));
        let prog = std::sync::Arc::new(std::sync::Mutex::new((0, 0, 0, 0)));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        install_release(&url, &dest, &prog, &stop).expect("install_release failed");
        assert!(
            std::fs::read_dir(&dest).map(|rd| rd.count()).unwrap_or(0) > 0,
            "{repo}: {name} unpacked into an empty dir"
        );
    }
}
/// Live smoke test against the real store API. Run manually with a
/// bearer in the environment, never in CI:
/// `META_TEST_BEARER=<token> cargo test --release -- --ignored`
#[test]
#[ignore]
fn live_store_paths() {
    let Ok(bearer) = std::env::var("META_TEST_BEARER") else {
        panic!("set META_TEST_BEARER to run");
    };
    let apps = owned_apps(bearer.trim(), 10).expect("owned_apps failed");
    assert!(
        apps.iter().any(|a| a.entitled && !a.binary_id.is_empty()),
        "no entitled app with a binary id"
    );
    let channels = release_channels(bearer.trim(), "2385436581584047").expect("channels");
    assert!(channels
        .iter()
        .any(|b| b.name == "LIVE" && !b.binary_id.is_empty()));
    let meta = app_info(bearer.trim(), "2385436581584047").expect("app_info failed");
    assert!(
        !meta.cover_url.is_empty(),
        "no title art in store format doc"
    );
    let dest = std::env::temp_dir().join("meta-titleart-probe.img");
    download_cover(&meta.cover_url, &dest).expect("cover download failed");
    let img = image::load_from_memory(&std::fs::read(&dest).unwrap())
        .expect("cover not a decodable image");
    assert!(img.width() > 0 && img.height() > 0);
    std::fs::remove_file(&dest).ok();
}
#[test]
fn proton_arch_filter_drops_aarch64() {
    // GE-Proton ships per-arch tarballs; the offer list must never hand the
    // user an aarch64 build on an x86_64 machine.
    let assets = serde_json::json!([
        {"name": "GE-Proton10-29-aarch64.tar.gz", "browser_download_url": "u"},
        {"name": "GE-Proton10-29.tar.gz", "browser_download_url": "u2"},
    ]);
    let a = assets.as_array().unwrap();
    let picked = pick_asset(a, &[], &["win", "mac", "aarch64", "arm64", "arm"]).unwrap();
    assert_eq!(picked["name"].as_str().unwrap(), "GE-Proton10-29.tar.gz");
}
#[test]
fn incomplete_zip_gets_a_clear_truncation_message() {
    let half = b"PK\x03\x04 fake zip bytes without end".to_vec();
    let e = unpack_zip(half, std::path::Path::new("/tmp/never-used")).expect_err("must refuse");
    assert!(e.contains("incomplete"), "got: {e}");
    assert!(e.contains("download was cut off"), "got: {e}");
}
#[test]
fn truncated_asset_download_is_reported_not_accepted() {
    // A server that announces 100 bytes and closes after 50 must surface an
    // honest early-end error, not a half archive handed to the zip parser.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            use std::io::Write;
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n");
            let _ = stream.write_all(&[b'0'; 50]);
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    let prog = std::sync::Arc::new(std::sync::Mutex::new((0usize, 0usize, 0u64, 0u64)));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let e = fetch_bytes_watch(&format!("http://127.0.0.1:{port}/asset.zip"), &prog, &stop)
        .expect_err("truncated stream must not be accepted");
    assert!(e.contains("ended early"), "got: {e}");
    assert!(e.contains("50 of 100"), "got: {e}");
}
#[test]
#[ignore]
fn real_melonloader_asset_downloads_and_unpacks() {
    // Network regression for the truncation bug: the asset must unpack fully.
    let dest = std::env::temp_dir().join(format!("melon-live-{}", std::process::id()));
    let prog = std::sync::Arc::new(std::sync::Mutex::new((0usize, 0usize, 0u64, 0u64)));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    install_release(
        "https://github.com/LavaGang/MelonLoader/releases/download/v0.7.3/MelonLoader.x64.zip",
        &dest,
        &prog,
        &stop,
    )
    .expect("asset must unpack");
    assert!(dest.join("version.dll").is_file(), "loader proxy expected");
    assert!(dest.join("MelonLoader").is_dir(), "loader dir expected");
    std::fs::remove_dir_all(dest).ok();
}
#[test]
fn release_assets_follow_the_running_os() {
    let assets = serde_json::json!([
        {"name": "MelonLoader.Linux.x64.zip"},
        {"name": "MelonLoader.x64.zip"},
        {"name": "MelonLoader.mac.zip"},
    ]);
    let arr = assets.as_array().unwrap();
    assert_eq!(
        pick_asset(arr, &["linux"], &["win", "mac"]).unwrap()["name"],
        "MelonLoader.Linux.x64.zip"
    );
    assert_eq!(
        pick_asset(arr, &["win", "windows"], &["linux", "mac"]).unwrap()["name"],
        "MelonLoader.x64.zip"
    );
    let proton = serde_json::json!([
        {"name": "GE-Proton10-29-win.tar.gz"},
        {"name": "GE-Proton10-29-x86_64.tar.gz"},
    ]);
    let parr = proton.as_array().unwrap();
    assert_eq!(
        pick_asset(parr, &["linux"], &["win", "mac"]).unwrap()["name"],
        "GE-Proton10-29-x86_64.tar.gz"
    );
}
#[test]
fn network_blips_are_not_reported_as_token_rejections() {
    assert!(transport_blip("request failed: timed out reading the body"));
    assert!(transport_blip("read: connection reset"));
    assert!(transport_blip("json: expected value at line 1"));
    // A real refusal must still read as "log in again".
    assert!(!transport_blip("the store refused the request (code 400)"));
    assert!(!transport_blip("Failed to decrypt access token"));
}

#[test]
fn truncated_asset_gets_redownloaded_before_giving_up() {
    // A server that delivers exactly what it announces but whose body is not
    // a real zip: the watch accepts the clean stream, the zip guard catches
    // the cut archive, and install_release must re-fetch instead of giving up
    // after one pass. Announcing the true 50 bytes keeps this deterministic
    // on Windows runners, where mismatched Content-Length reconnects reset.
    let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let cnt = served.clone();
    std::thread::spawn(move || {
        // Per-connection threads and a short keep-alive after the body: wine's
        // loopback resets immediate closes, which hid the retry verdict behind
        // a connection error on the Windows test lanes.
        for stream in listener.incoming().flatten() {
            let cnt = cnt.clone();
            std::thread::spawn(move || {
                use std::io::Write;
                cnt.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let mut s = stream;
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 50\r\n\r\n",
                );
                let _ = s.write_all(&[b'0'; 50]);
                let _ = s.flush();
                std::thread::sleep(std::time::Duration::from_millis(1500));
                drop(s);
            });
        }
    });
    let dest = std::env::temp_dir().join(format!("iris-retry-{}", std::process::id()));
    std::fs::remove_dir_all(&dest).ok();
    let prog = std::sync::Arc::new(std::sync::Mutex::new((0usize, 0usize, 0u64, 0u64)));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let e = crate::fetcher::install_release(
        &format!("http://127.0.0.1:{port}/asset.zip"),
        &dest,
        &prog,
        &stop,
    )
    .expect_err("a cut archive must never be accepted");
    let served_now = served.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        e.contains("retried 3 times"),
        "got: {e} (connections served: {served_now})"
    );
    assert!(e.contains("cut off"), "got: {e}");
    assert!(
        served.load(std::sync::atomic::Ordering::Relaxed) >= 3,
        "the fetch must have been attempted at least 3 times"
    );
    std::fs::remove_dir_all(&dest).ok();
}
