use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

/// Return-kind of a typed default stub, parsed from the SDK headers into
/// docs/platform_export_types.tsv. Unclassified and Voip exports keep the
/// generic handle stub: guessing a scalar return for an unknown signature
/// would hand the game garbage where it expects a number.
fn stub(n: &str, kind: &str) -> String {
    match kind {
        "void" => format!("#[no_mangle]\npub extern \"C\" fn {n}() {{ record(\"{n}\"); }}\n"),
        "str" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> *mut u8 {{ record(\"{n}\"); empty_str() }}\n"),
        "u64" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> u64 {{ record(\"{n}\"); 0 }}\n"),
        "i64" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> i64 {{ record(\"{n}\"); 0 }}\n"),
        "u32" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> u32 {{ record(\"{n}\"); 0 }}\n"),
        "i32" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> i32 {{ record(\"{n}\"); 0 }}\n"),
        "bool" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> bool {{ record(\"{n}\"); false }}\n"),
        "f32" => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> f32 {{ record(\"{n}\"); 0.0 }}\n"),
        _ => format!("#[no_mangle]\npub extern \"C\" fn {n}() -> *mut u8 {{ record(\"{n}\"); handle() }}\n"),
    }
}


fn main() {
    let exports_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/platform_exports.txt");
    let names: Vec<String> = fs::read_to_string(&exports_path)
        .expect("platform_exports.txt missing")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l.starts_with("ovr"))
        .collect();

    let types_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/platform_export_types.tsv");
    let kinds: HashMap<String, String> = fs::read_to_string(&types_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (n, k) = l.split_once('\t')?;
            Some((n.to_string(), k.to_string()))
        })
        .collect();


    // Header the generated file carries so nobody mistakes generic stubs for
    // real ABI: unhandled exports take no arguments and only trace + handle.
    let mut gen = String::from(
        "// GENERATED STUBS: exports not handled below take no arguments and\n// return the shim's generic message handle. Do not rely on their\n// signatures; add a hand-written arm here when a game calls one for real.\n",
    );
    for n in &names {
        match n.as_str() {
            "ovr_UnityInitWrapperWindows" | "ovr_UnityInitWrapperWindowsAsynchronous"
            | "ovr_UnityInitWrapperStandalone" | "ovr_UnityInitGlobals" => gen.push_str(
                &format!(
                    "#[no_mangle]\npub extern \"C\" fn {n}() {{ record(\"{n}\"); init(); }}\n",
                    n = n
                ),
            ),
            "ovr_UnityPutMessageOnQueue" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_UnityPutMessageOnQueue(msg: *mut crate::Msg) { let _ = msg; record(\"ovr_UnityPutMessageOnQueue\"); }\n",
            ),
            "ovr_Message_GetString" | "ovr_Message_GetStringForJavascript"
            | "ovr_Error_GetMessage" | "ovr_Error_GetDisplayableMessage" => gen.push_str(
                &format!(
                    "#[no_mangle]\npub extern \"C\" fn {n}() -> *mut u8 {{ record(\"{n}\"); empty_str() }}\n",
                    n = n
                ),
            ),
            "ovr_Error_GetCode" | "ovr_Error_GetHttpCode" => gen.push_str(
                &format!(
                    "#[no_mangle]\npub extern \"C\" fn {n}() -> u32 {{ record(\"{n}\"); 0 }}\n",
                    n = n
                ),
            ),
            "ovr_Message_GetRequestID" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_GetRequestID(msg: *mut crate::Msg) -> u64 { record(\"ovr_Message_GetRequestID\"); msg_id(msg) }\n",
            ),
            // FreeMessage stays untraced (the pop line already covers the
            // message) and deliberately leaks the queued Msg: the queue holds
            // exactly one message per game, freed at process exit. Box::from_raw
            // here would double-free whenever a caller passes a pointer we did
            // not hand out.
            "ovr_FreeMessage" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_FreeMessage(msg: *mut crate::Msg) { let _ = msg; }\n",
            ),
            "ovr_Entitlement_GetIsViewerEntitled" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Entitlement_GetIsViewerEntitled() -> u64 { record(\"ovr_Entitlement_GetIsViewerEntitled\"); entitlement_request() }\n",
            ),
            "ovr_PopMessage" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_PopMessage() -> *mut crate::Msg { let m = pop_message(); record_pop(m); m }\n",
            ),
            "ovr_Message_GetType" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_GetType(msg: *mut crate::Msg) -> u32 { if !msg.is_null() { record(&format!(\"ovr_Message_GetType type={:08X}\", msg_type(msg))); } else { record(\"ovr_Message_GetType null\"); } msg_type(msg) }\n",
            ),
            "ovr_Message_IsError" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_IsError(msg: *mut crate::Msg) -> u32 { if !msg.is_null() { record(&format!(\"ovr_Message_IsError {}\", msg_is_error(msg))); } msg_is_error(msg) }\n",
            ),
            "ovr_Message_GetCompleted" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_GetCompleted(msg: *mut crate::Msg) -> *mut crate::Msg { record(\"ovr_Message_GetCompleted\"); msg_completed(msg) }\n",
            ),
            "ovr_Message_GetRequest" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_GetRequest(msg: *mut crate::Msg) -> *mut crate::Msg { let _ = msg; record(\"ovr_Message_GetRequest\"); std::ptr::null_mut() }\n",
            ),
            "ovr_Message_GetError" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_GetError(msg: *mut crate::Msg) -> *mut crate::Msg { let _ = msg; record(\"ovr_Message_GetError\"); std::ptr::null_mut() }\n",
            ),
            "ovr_Entitlement_GetIsViewerEntitled_Result_GetIsEntitled" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Entitlement_GetIsViewerEntitled_Result_GetIsEntitled(msg: *mut crate::Msg) -> u32 { record(\"ovr_Entitlement_GetIsViewerEntitled_Result_GetIsEntitled\"); msg_entitled(msg) }\n",
            ),
            // Bonelab unlocks achievements through this exact export (seen
            // in the live trace). The id must reach the trace, not just the
            // call name; that is the future achievements section's feed.
            "ovr_Achievements_Unlock" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Achievements_Unlock(id: *const u8) -> *mut u8 { record(&format!(\"ovr_Achievements_Unlock {}\", crate::cstr(id))); handle() }\n",
            ),
            "ovr_Leaderboard_Get" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_Get() -> u64 { record(\"ovr_Leaderboard_Get\"); leaderboard_request(0x6AD44EF8) }\n",
            ),
            "ovr_Leaderboard_GetEntries" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_GetEntries() -> u64 { record(\"ovr_Leaderboard_GetEntries\"); leaderboard_request(0x5DB3474C) }\n",
            ),
            "ovr_Leaderboard_GetEntriesAfterRank" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_GetEntriesAfterRank() -> u64 { record(\"ovr_Leaderboard_GetEntriesAfterRank\"); leaderboard_request(0x18378BEF) }\n",
            ),
            "ovr_Leaderboard_GetEntriesByIds" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_GetEntriesByIds() -> u64 { record(\"ovr_Leaderboard_GetEntriesByIds\"); leaderboard_request(0x39607BFC) }\n",
            ),
            "ovr_Leaderboard_GetNextEntries" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_GetNextEntries() -> u64 { record(\"ovr_Leaderboard_GetNextEntries\"); leaderboard_request(0x4E207CD9) }\n",
            ),
            "ovr_Leaderboard_GetPreviousEntries" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_GetPreviousEntries() -> u64 { record(\"ovr_Leaderboard_GetPreviousEntries\"); leaderboard_request(0x4901DAC0) }\n",
            ),
            "ovr_Leaderboard_GetNextLeaderboardArrayPage" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_GetNextLeaderboardArrayPage() -> u64 { record(\"ovr_Leaderboard_GetNextLeaderboardArrayPage\"); leaderboard_request(0x35F6769B) }\n",
            ),
            "ovr_Leaderboard_WriteEntry" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_WriteEntry() -> u64 { record(\"ovr_Leaderboard_WriteEntry\"); leaderboard_request(0x117FC8FE) }\n",
            ),
            "ovr_Leaderboard_WriteEntryWithSupplementaryMetric" => gen.push_str(
                "#[no_mangle]\npub extern \"C\" fn ovr_Leaderboard_WriteEntryWithSupplementaryMetric() -> u64 { record(\"ovr_Leaderboard_WriteEntryWithSupplementaryMetric\"); leaderboard_request(0x72C692FA) }\n",
            ),
            _ => {
                let kind = if n.starts_with("ovr_Voip") {
                    "handle"
                } else {
                    kinds.get(n).map(|s| s.as_str()).unwrap_or("handle")
                };
                gen.push_str(&stub(n, kind));
            }
        }
    }

    let out = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out).join("generated.rs"), gen).unwrap();
    println!("cargo:rerun-if-changed={}", exports_path.display());
    println!("cargo:rerun-if-changed={}", types_path.display());
    println!("cargo:info={} exports generated", names.len());
}
