use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let exports_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/rt_exports.txt");
    let names: Vec<String> = fs::read_to_string(&exports_path)
        .expect("rt_exports.txt missing")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l.starts_with("ovr"))
        .collect();

    let mut gen = String::new();
    for n in &names {
        gen.push_str(&format!(
            "#[no_mangle]\npub extern \"C\" fn {n}() -> *mut u8 {{ record(\"{n}\"); handle() }}\n"
        ));
    }

    let out = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out).join("generated.rs"), gen).unwrap();
    println!("cargo:rerun-if-changed={}", exports_path.display());
    println!("cargo:info={} exports generated", names.len());
}
