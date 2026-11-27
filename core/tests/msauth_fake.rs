//! Runs the whole Microsoft -> Xbox -> XSTS -> Minecraft chain against a local fake server.

use solocraft_core::{msauth, Engine};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

fn read_request(s: &mut std::net::TcpStream) -> (String, String, String) {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = s.read(&mut buf).unwrap_or(0);
        if n == 0 { break; }
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data).to_string();
        if let Some(pos) = text.find("\r\n\r\n") {
            let len = text.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            if data.len() >= pos + 4 + len { break; }
        }
    }
    let text = String::from_utf8_lossy(&data).to_string();
    let first = text.lines().next().unwrap_or("").to_string();
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (path, body, text)
}

#[tokio::test]
async fn microsoft_login_chain() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let polls = Arc::new(AtomicUsize::new(0));
    let p2 = polls.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut s = match stream { Ok(s) => s, Err(_) => continue };
            let (path, body, full) = read_request(&mut s);
            let (status, json) = match path.as_str() {
                "/devicecode" => (200, serde_json::json!({"device_code":"dev123","user_code":"ABCD-EFGH","verification_uri":"https://www.microsoft.com/link","expires_in":60,"interval":1})),
                "/token" if body.contains("grant_type=refresh_token") => {
                    if body.contains("refresh_token=badrefresh") {
                        (400, serde_json::json!({"error":"invalid_grant"}))
                    } else {
                        (200, serde_json::json!({"access_token":"ms-access-2","refresh_token":"refresh-2"}))
                    }
                }
                "/token" => {
                    if p2.fetch_add(1, Ordering::SeqCst) == 0 {
                        (400, serde_json::json!({"error":"authorization_pending"}))
                    } else {
                        (200, serde_json::json!({"access_token":"ms-access","refresh_token":"refresh-1"}))
                    }
                }
                "/xbl" => (200, serde_json::json!({"Token":"xbl-token","DisplayClaims":{"xui":[{"uhs":"uhs123"}]}})),
                "/xsts" => (200, serde_json::json!({"Token":"xsts-token","DisplayClaims":{"xui":[{"uhs":"uhs123"}]}})),
                "/mc/login" => {
                    if body.contains("XBL3.0 x=uhs123;xsts-token") {
                        (200, serde_json::json!({"access_token":"mc-token-xyz","expires_in":86400}))
                    } else {
                        (400, serde_json::json!({"error":"bad identity token"}))
                    }
                }
                "/mc/profile" => {
                    if full.contains("Bearer mc-token-xyz") {
                        (200, serde_json::json!({"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch"}))
                    } else {
                        (401, serde_json::json!({}))
                    }
                }
                _ => (404, serde_json::json!({})),
            };
            let b = json.to_string();
            let _ = s.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).as_bytes());
        }
    });
    std::env::set_var("SOLOCRAFT_AUTH_TEST_BASE", format!("http://127.0.0.1:{port}"));

    // no client id -> clear error before any network call
    assert!(msauth::begin_device_login("").await.unwrap_err().to_string().contains("client"));

    let dir = std::env::temp_dir().join(format!("sc-ms-test-{port}"));
    let _ = std::fs::remove_dir_all(&dir);
    let engine = Engine::new(dir.clone()).unwrap();
    let store = &engine.store;
    let mut settings = store.load_settings();
    settings.ms_client_id = "client-abc".into();
    store.save_settings(&settings).unwrap();

    let dc = msauth::begin_device_login("client-abc").await.unwrap();
    assert_eq!(dc.user_code, "ABCD-EFGH");
    let cancel = AtomicBool::new(false);
    let tokens = msauth::poll_device_login("client-abc", &dc, &cancel).await.unwrap();
    assert_eq!(tokens.access, "ms-access");
    assert!(polls.load(Ordering::SeqCst) >= 2, "must keep polling while pending");

    let session = msauth::minecraft_session(&tokens.access).await.unwrap();
    assert_eq!(session.username, "Notch");
    assert_eq!(session.uuid, "069a79f444e94726a5befca90e38aaf5");

    let acc = msauth::save_login(store, &tokens, &session).unwrap();
    assert_eq!(acc.kind, "microsoft");
    assert_eq!(store.load_accounts().active.as_deref(), Some(acc.id.as_str()));

    // token still fresh -> returned without any refresh
    let (t, _) = msauth::ensure_token(store, &acc).await.unwrap();
    assert_eq!(t, "mc-token-xyz");

    // verify runs the full refresh chain and rewrites the stored refresh token
    let v = msauth::verify(store, &acc).await.unwrap();
    assert_eq!(v.username, "Notch");
    let secrets = std::fs::read_to_string(dir.join("ms_secrets.json")).unwrap();
    assert!(secrets.contains("refresh-2"));

    // cancelling stops polling
    let stop = AtomicBool::new(true);
    assert!(msauth::poll_device_login("client-abc", &dc, &stop).await.unwrap_err().to_string().contains("cancel"));

    // removing the account also removes its stored tokens
    store.remove_account(&acc.id).unwrap();
    let secrets = std::fs::read_to_string(dir.join("ms_secrets.json")).unwrap();
    assert!(!secrets.contains("refresh-2"));
    let _ = std::fs::remove_dir_all(&dir);
}
