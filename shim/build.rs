use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let exports_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/platform_exports.txt");
    let names: Vec<String> = fs::read_to_string(&exports_path)
        .expect("platform_exports.txt missing")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l.starts_with("ovr"))
        .collect();

    // Header the generated file carries so nobody mistakes generic stubs for
    // real ABI: unhandled exports take no arguments and only trace + handle.
    let mut gen = String::from(
        "// GENERATED STUBS: exports not handled below take no arguments and \n"
        "// return the shim's generic message handle. Do not rely on their \n"
        "// signatures; add a hand-written arm here when a game calls one for real.\n",
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
                "#[no_mangle]\npub extern \"C\" fn ovr_Message_GetRequestID(msg: *mut crate::Msg) -> u32 { record(\"ovr_Message_GetRequestID\"); msg_id(msg) }\n",
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
                "#[no_mangle]\npub extern \"C\" fn ovr_Entitlement_GetIsViewerEntitled() -> u32 { record(\"ovr_Entitlement_GetIsViewerEntitled\"); entitlement_request() }\n",
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
            _ => gen.push_str(&format!(
                "#[no_mangle]\npub extern \"C\" fn {n}() -> *mut u8 {{ record(\"{n}\"); handle() }}\n"
            )),
        }
    }

    let out = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out).join("generated.rs"), gen).unwrap();
    println!("cargo:rerun-if-changed={}", exports_path.display());
    println!("cargo:info={} exports generated", names.len());
}
