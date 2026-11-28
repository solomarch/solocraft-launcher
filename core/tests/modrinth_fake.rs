//! End-to-end test of the Modrinth engine against a local fake server.

use sha1::{Digest as _, Sha1};
use sha2::Sha512;
use solocraft_core::events::EventSink;
use solocraft_core::models::{GameState, LogLine, Progress};
use solocraft_core::{modrinth, Engine};
use std::io::{Read, Write};
use std::net::TcpListener;

struct Quiet;
impl EventSink for Quiet {
    fn progress(&self, _: Progress) {}
    fn log(&self, _: LogLine) {}
    fn state(&self, _: GameState) {}
}

fn hex1(b: &[u8]) -> String { hex::encode(Sha1::digest(b)) }
fn hex512(b: &[u8]) -> String { hex::encode(Sha512::digest(b)) }

fn make_mrpack(port: u16, bad_hash: bool, evil_path: bool) -> Vec<u8> {
    let mod_bytes = b"fake-sodium-jar".to_vec();
    let h512 = if bad_hash { "0".repeat(128) } else { hex512(&mod_bytes) };
    let path = if evil_path { "../../escape.jar" } else { "mods/sodium.jar" };
    let index = serde_json::json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0", "name": "Test Pack",
        "files": [{
            "path": path,
            "hashes": {"sha1": hex1(&mod_bytes), "sha512": h512},
            "env": {"client": "required", "server": "required"},
            "downloads": [format!("http://127.0.0.1:{port}/files/sodium.jar")],
            "fileSize": mod_bytes.len()
        }, {
            "path": "mods/server-only.jar",
            "hashes": {"sha1": "00", "sha512": "00"},
            "env": {"client": "unsupported", "server": "required"},
            "downloads": [format!("http://127.0.0.1:{port}/files/none.jar")],
            "fileSize": 1
        }],
        "dependencies": {"minecraft": "1.21.4", "fabric-loader": "0.16.9"}
    });
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("modrinth.index.json", o).unwrap();
        z.write_all(index.to_string().as_bytes()).unwrap();
        z.start_file("overrides/config/pack.txt", o).unwrap();
        z.write_all(b"from overrides").unwrap();
        z.start_file("client-overrides/config/pack.txt", o).unwrap();
        z.write_all(b"from client-overrides").unwrap();
        z.start_file("overrides/../../zipslip.txt", o).unwrap();
        z.write_all(b"evil").unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn version_json(port: u16, id: &str, project: &str, file: &str, bytes: &[u8], deps: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": id, "project_id": project, "name": id, "version_number": "1.0.0",
        "version_type": "release", "game_versions": ["1.21.4"], "loaders": ["fabric"],
        "date_published": "2026-01-01T00:00:00Z", "downloads": 1,
        "files": [{"url": format!("http://127.0.0.1:{port}/files/{file}"), "filename": file,
                   "primary": true, "size": bytes.len(),
                   "hashes": {"sha1": hex1(bytes), "sha512": hex512(bytes)}}],
        "dependencies": deps
    })
}

fn serve(listener: TcpListener, port: u16) {
    let sodium = b"fake-sodium-jar".to_vec();
    let fabric_api = b"fake-fabric-api".to_vec();
    let lithium = b"fake-lithium".to_vec();
    let good_pack = make_mrpack(port, false, false);
    let bad_pack = make_mrpack(port, true, false);
    let evil_pack = make_mrpack(port, false, true);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut s = match stream { Ok(s) => s, Err(_) => continue };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let line = req.lines().next().unwrap_or("").to_string();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let (ctype, body): (&str, Vec<u8>) = if path.starts_with("/files/sodium.jar") {
                ("application/java-archive", sodium.clone())
            } else if path.starts_with("/files/fabric-api.jar") {
                ("application/java-archive", fabric_api.clone())
            } else if path.starts_with("/files/lithium.jar") {
                ("application/java-archive", lithium.clone())
            } else if path.starts_with("/files/good.mrpack") {
                ("application/zip", good_pack.clone())
            } else if path.starts_with("/files/bad.mrpack") {
                ("application/zip", bad_pack.clone())
            } else if path.starts_with("/files/evil.mrpack") {
                ("application/zip", evil_pack.clone())
            } else if path.starts_with("/v2/project/sodium/version") {
                let deps = serde_json::json!([{"project_id": "fabric-api", "dependency_type": "required"},
                                              {"project_id": "optional-thing", "dependency_type": "optional"}]);
                ("application/json", serde_json::json!([version_json(port, "v-sodium", "sodium", "sodium.jar", &sodium, deps)]).to_string().into_bytes())
            } else if path.starts_with("/v2/project/fabric-api/version") {
                ("application/json", serde_json::json!([version_json(port, "v-fapi", "fabric-api", "fabric-api.jar", &fabric_api, serde_json::json!([]))]).to_string().into_bytes())
            } else if path.starts_with("/v2/project/lithium/version") {
                ("application/json", serde_json::json!([]).to_string().into_bytes()) // no build: must be skipped
            } else if path.starts_with("/v2/project/packy/version") {
                let mut v = version_json(port, "v-pack", "packy", "good.mrpack", &good_pack, serde_json::json!([]));
                v["files"][0]["url"] = format!("http://127.0.0.1:{port}/files/good.mrpack").into();
                ("application/json", serde_json::json!([v]).to_string().into_bytes())
            } else if path.starts_with("/v2/projects") {
                ("application/json", serde_json::json!([
                    {"id":"sodium","slug":"sodium","title":"Sodium"},
                    {"id":"fabric-api","slug":"fabric-api","title":"Fabric API"},
                    {"id":"packy","slug":"packy","title":"Packy"}]).to_string().into_bytes())
            } else if path.starts_with("/v2/project/packy") {
                ("application/json", serde_json::json!({"id":"packy","slug":"packy","title":"Packy","description":"d","body":"b"}).to_string().into_bytes())
            } else if path.starts_with("/v2/version_files") {
                ("application/json", b"{}".to_vec())
            } else if path.starts_with("/v2/search") {
                ("application/json", serde_json::json!({"hits":[{"project_id":"sodium","slug":"sodium","title":"Sodium","description":"fast","downloads":5,"follows":2,"author":"jelly","categories":["fabric"],"project_type":"mod"}],"offset":0,"limit":20,"total_hits":1}).to_string().into_bytes())
            } else {
                let r = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = s.write_all(r);
                continue;
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
        }
    });
}

#[tokio::test]
async fn modrinth_end_to_end() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    serve(listener, port);
    std::env::set_var("MODRINTH_API", format!("http://127.0.0.1:{port}/v2"));

    let dir = std::env::temp_dir().join(format!("sc-mr-test-{port}"));
    let _ = std::fs::remove_dir_all(&dir);
    let engine = Engine::new(dir.clone()).unwrap();
    let store = &engine.store;
    let sink = Quiet;

    // search
    let r = modrinth::search(&modrinth::SearchQuery {
        query: "sodium".into(), project_type: "mod".into(), mc_version: Some("1.21.4".into()),
        loader: Some("fabric".into()), categories: vec![], sort: "downloads".into(), offset: 0, limit: 20,
    }).await.unwrap();
    assert_eq!(r.hits[0].title, "Sodium");

    // mod install pulls the required dependency, not the optional one
    let mut inst = store.create_instance("Fabric Test", "1.21.4", "").unwrap();
    inst.loader = "fabric".into();
    store.save_instance(&inst).unwrap();
    let plan = modrinth::plan_mod_install(&engine.paths(), &inst, "sodium").await.unwrap();
    assert_eq!(plan.len(), 2);
    let done = modrinth::install_mod(&engine.paths(), &inst, "sodium", &sink).await.unwrap();
    assert_eq!(done.len(), 2);
    let mods = engine.paths().game_dir(&inst.id).join("mods");
    assert!(mods.join("sodium.jar").is_file());
    assert!(mods.join("fabric-api.jar").is_file());
    let listed = modrinth::list_mods(&engine.paths(), &inst.id).await.unwrap();
    assert_eq!(listed.len(), 2);
    let sod = listed.iter().find(|m| m.filename == "sodium.jar").unwrap();
    assert_eq!(sod.title, "Sodium");
    assert!(sod.explicit);
    assert!(!listed.iter().find(|m| m.filename == "fabric-api.jar").unwrap().explicit);

    // installing again plans only what is missing
    let again = modrinth::plan_mod_install(&engine.paths(), &inst, "sodium").await.unwrap();
    assert!(again.is_empty());

    // disable / enable / remove
    modrinth::set_mod_enabled(&engine.paths(), &inst.id, "sodium.jar", false).unwrap();
    assert!(mods.join("sodium.jar.disabled").is_file());
    modrinth::set_mod_enabled(&engine.paths(), &inst.id, "sodium.jar.disabled", true).unwrap();
    assert!(mods.join("sodium.jar").is_file());
    assert!(modrinth::set_mod_enabled(&engine.paths(), &inst.id, "../x.jar", false).is_err());
    modrinth::remove_mod(&engine.paths(), &inst.id, "fabric-api.jar").unwrap();
    assert!(!mods.join("fabric-api.jar").exists());

    // FPS pack: lithium has no build and is skipped, the rest are reported
    let fresh = store.create_instance("Fps Test", "1.21.4", "").unwrap();
    let mut fresh = fresh;
    fresh.loader = "fabric".into();
    store.save_instance(&fresh).unwrap();
    let rep = modrinth::install_fps_stack(&engine.paths(), &fresh, &sink).await.unwrap();
    assert!(rep.installed.iter().any(|m| m.project_id == "sodium"));
    assert!(rep.skipped.contains(&"lithium".to_string()));

    // a vanilla instance cannot take mods
    let van = store.create_instance("Vanilla", "1.21.4", "").unwrap();
    assert!(modrinth::plan_mod_install(&engine.paths(), &van, "sodium").await.is_err());

    // modpack install: good pack, overrides layered, server-only file skipped
    let pack = modrinth::install_modpack(store, "packy", None, None, "Modpacks", &sink).await.unwrap();
    assert_eq!(pack.loader, "fabric");
    assert_eq!(pack.loader_version, "0.16.9");
    assert_eq!(pack.mc_version, "1.21.4");
    let gd = engine.paths().game_dir(&pack.id);
    assert_eq!(std::fs::read(gd.join("mods/sodium.jar")).unwrap(), b"fake-sodium-jar");
    assert!(!gd.join("mods/server-only.jar").exists());
    assert_eq!(std::fs::read_to_string(gd.join("config/pack.txt")).unwrap(), "from client-overrides");
    assert!(!dir.join("zipslip.txt").exists());
    assert!(!dir.parent().unwrap().join("zipslip.txt").exists());

    // damaged / hostile packs fail cleanly and leave no instance behind
    let before = store.list_instances().len();
    for name in ["bad", "evil"] {
        let p = dir.join(format!("{name}.mrpack"));
        let bytes = reqwest::get(format!("http://127.0.0.1:{port}/files/{name}.mrpack")).await.unwrap().bytes().await.unwrap();
        std::fs::write(&p, &bytes).unwrap();
        let err = modrinth::install_mrpack_file(store, &p, None, "", &sink, "t").await;
        assert!(err.is_err(), "{name} pack must fail");
        assert_eq!(store.list_instances().len(), before, "{name} pack left an instance");
    }
    assert!(!dir.join("instances").join("escape.jar").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
