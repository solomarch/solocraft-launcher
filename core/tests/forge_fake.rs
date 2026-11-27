//! Runs a tiny fake "installer" jar through the same code path used for the
//! real Forge/NeoForge installers (needs a JDK; skipped when javac is absent).
use solocraft_core::events::EventSink;
use solocraft_core::models::{GameState, LogLine, Progress};
use solocraft_core::{forge, Paths};
use std::process::Command;
use std::sync::Mutex;

struct Sink(Mutex<Vec<String>>);
impl EventSink for Sink {
    fn progress(&self, _p: Progress) {}
    fn log(&self, l: LogLine) {
        self.0.lock().unwrap().push(l.text);
    }
    fn state(&self, _s: GameState) {}
}

const SRC: &str = r#"import java.nio.file.*;
public class FakeInstaller { public static void main(String[] a) throws Exception {
  if (!a[0].equals("--installClient")) System.exit(3);
  Path d = Paths.get(a[1]);
  if (!Files.exists(d.resolve("launcher_profiles.json"))) System.exit(4);
  if (!Files.exists(d.resolve("versions/1.21.4/1.21.4.json"))) System.exit(5);
  Files.createDirectories(d.resolve("versions/1.21.4-forge-9.9"));
  Files.writeString(d.resolve("versions/1.21.4-forge-9.9/1.21.4-forge-9.9.json"), "{\"id\":\"1.21.4-forge-9.9\"}");
  Files.createDirectories(d.resolve("libraries/a/b/1"));
  Files.writeString(d.resolve("libraries/a/b/1/b-1.jar"), "x");
  System.out.println("Processor finished");
}}"#;

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-version").output().is_ok()
}

#[tokio::test]
async fn installer_output_is_collected() {
    if !have("javac") || !have("jar") {
        eprintln!("skipped: no JDK");
        return;
    }
    let base = std::env::temp_dir().join(format!("sc-forge-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    struct Tmp(std::path::PathBuf);
    impl Tmp { fn path(&self) -> &std::path::Path { &self.0 } }
    let tmp = Tmp(base);
    let work = tmp.path().join("w");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("FakeInstaller.java"), SRC).unwrap();
    assert!(Command::new("javac").arg("FakeInstaller.java").current_dir(&work).status().unwrap().success());
    assert!(Command::new("jar").args(["cfe", "inst.jar", "FakeInstaller", "FakeInstaller.class"]).current_dir(&work).status().unwrap().success());

    let paths = Paths::new(tmp.path().join("root"));
    paths.ensure_base().unwrap();
    // pretend vanilla is installed
    std::fs::create_dir_all(paths.version_dir("1.21.4")).unwrap();
    std::fs::write(paths.version_json("1.21.4"), "{}").unwrap();
    std::fs::write(paths.client_jar("1.21.4"), "jar").unwrap();

    let sink = Sink(Mutex::new(vec![]));
    let id = forge::installed_id("forge", "1.21.4", "9.9");
    forge::run_installer(&paths, &id, "1.21.4", &work.join("inst.jar"), "java", &sink, "t", "Forge")
        .await
        .unwrap();
    assert!(paths.version_json(&id).exists());
    assert!(paths.libraries().join("a/b/1/b-1.jar").exists());
    assert!(!paths.root.join("tmp").join(format!("loader-{id}")).exists());
    assert!(sink.0.lock().unwrap().iter().any(|l| l.contains("Processor finished")));

    // a failing installer reports its exit code
    let bad = forge::run_installer(&paths, "x", "9.9.9", &work.join("inst.jar"), "java", &sink, "t", "Forge").await;
    assert!(bad.unwrap_err().to_string().contains("failed"));
}

#[tokio::test]
async fn prism_zip_imports_into_instance() {
    use std::io::Write;
    let base = std::env::temp_dir().join(format!("sc-prism-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let zp = base.join("p.zip");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("Pack/mmc-pack.json", o).unwrap();
    z.write_all(br#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.15.11"}]}"#).unwrap();
    z.start_file("Pack/instance.cfg", o).unwrap();
    z.write_all(b"name=Imported Pack\n").unwrap();
    z.start_file("Pack/.minecraft/mods/a.jar", o).unwrap();
    z.write_all(b"jar").unwrap();
    z.start_file("Pack/.minecraft/../evil.txt", o).unwrap();
    z.write_all(b"no").unwrap();
    z.finish().unwrap();
    let store = solocraft_core::Store::new(Paths::new(base.join("root")));
    store.paths.ensure_base().unwrap();
    let sink = Sink(Mutex::new(vec![]));
    let inst = solocraft_core::import::import_file(&store, &zp, None, "", &sink, "t").await.unwrap();
    assert_eq!(inst.name, "Imported Pack");
    assert_eq!(inst.loader, "fabric");
    assert_eq!(inst.loader_version, "0.15.11");
    assert!(store.paths.game_dir(&inst.id).join("mods/a.jar").exists());
    assert!(!store.paths.instance_dir(&inst.id).join("evil.txt").exists());
}
