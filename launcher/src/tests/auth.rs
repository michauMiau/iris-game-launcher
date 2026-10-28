use crate::auth::*;
#[allow(unused_imports)]
#[test]
fn callback_params_parse() {
    let tok = "df3502998fd225f6";
    let url = format!("oculus://link/frl_login?token={tok}&blob=AbnI9xh9VzZMbL209bhX");
    let (t, b) = callback_params(&url).unwrap();
    assert_eq!(t, tok);
    assert_eq!(b, "AbnI9xh9VzZMbL209bhX");
    assert!(callback_params("oculus://link/frl_login?token=ABC&blob=X").is_none());
    assert!(callback_params("no-query-part").is_none());
}
#[test]
fn callback_params_lowercases_uppercase_hex_token() {
    let url = "oculus://link/frl_login?token=DF3502998FD225F6&blob=AbnI9xh9VzZMbL209bhX";
    let (t, b) = callback_params(url).unwrap();
    assert_eq!(t, "df3502998fd225f6");
    assert_eq!(b, "AbnI9xh9VzZMbL209bhX");
}
#[test]
fn callback_params_percent_decode() {
    let tok = "df3502998fd225f6";
    let url = "oculus://link/frl_login?token=%64%66%33%35%30%32%39%39%38%66%64%32%32%35%66%36&blob=a%2Dz%5F9";
    let (t, b) = callback_params(url).unwrap();
    assert_eq!(t, tok);
    assert_eq!(b, "a-z_9");
}
#[test]
fn callback_params_strips_stray_quotes() {
    // Real failure mode from launcher.log: handler arg ended with an apostrophe.
    let url = "oculus://link/frl_login?token=df3502998fd225f6&blob=AbnI9xh9VzZMbL209bhX'";
    let (t, b) = callback_params(url).unwrap();
    assert_eq!(t, "df3502998fd225f6");
    assert_eq!(b, "AbnI9xh9VzZMbL209bhX");
    let url2 = "\"oculus://link/frl_login?token=df3502998fd225f6&blob=AbnI9xh9\"";
    let (_, b2) = callback_params(url2).unwrap();
    assert_eq!(b2, "AbnI9xh9");
}
#[test]
fn callback_token_matches_gate() {
    let sso = "native-sso-token-abc";
    let mut hasher = Sha256::new();
    hasher.update(sso.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    assert!(callback_token_matches(sso, &hex[..16]));
    assert!(!callback_token_matches(sso, &hex[..16].replace('a', "b")));
    assert!(!callback_token_matches("other-token", &hex[..16]));
}
#[ignore]
#[test]
fn fake_credentials_surface_server_errors() {
    // Proves the exchange path reaches the vendor and surfaces honest errors.
    let e = exchange("FAKE_TOKEN", "FAKE_ETOKEN", "FAKE_BLOB").unwrap_err();
    assert!(e.contains("code"), "error was: {e}");
}
