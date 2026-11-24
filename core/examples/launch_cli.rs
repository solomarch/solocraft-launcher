//! Command-line runner for testing the launch pipeline without the GUI.
//! Usage: cargo run -p solocraft-core --example launch_cli -- <data-dir> <mc-version> <username>
//! Optional environment: SC_JVM_ARGS="-Dfoo=bar"  SC_KILL_AFTER=<seconds>

use solocraft_core::events::EventSink;
use solocraft_core::models::{GameState, LogLine, Progress};
use solocraft_core::Engine;
use std::path::PathBuf;
use std::sync::Arc;

struct Cli;

impl EventSink for Cli {
    fn progress(&self, p: Progress) {
        if p.total > 0 {
            println!("[progress] {}: {}/{}", p.stage, p.current, p.total);
        } else {
            println!("[progress] {}", p.stage);
        }
    }
    fn log(&self, l: LogLine) {
        println!("[{}] {}", l.level, l.text);
    }
    fn state(&self, s: GameState) {
        println!("[state] {} - {}", s.state, s.detail);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: launch_cli <data-dir> <mc-version> <username>");
        std::process::exit(2);
    }
    let engine = Engine::new(PathBuf::from(&args[1]))?;
    let mc = &args[2];
    let name = &args[3];

    if engine.store.load_accounts().accounts.is_empty() {
        engine.store.add_offline_account(name)?;
    }
    let mut inst = match engine.store.list_instances().into_iter().find(|i| &i.mc_version == mc) {
        Some(i) => i,
        None => engine.store.create_instance(&format!("test-{mc}"), mc, "")?,
    };
    if let Ok(j) = std::env::var("SC_JVM_ARGS") {
        inst.jvm_args = j;
        engine.store.save_instance(&inst)?;
    }
    if let Ok(s) = std::env::var("SC_KILL_AFTER") {
        if let Ok(secs) = s.parse::<u64>() {
            let e = engine.clone();
            let id = inst.id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                println!("[cli] sending kill");
                e.kill(&id);
            });
        }
    }
    let sink: Arc<dyn EventSink> = Arc::new(Cli);
    engine.launch(&inst.id, sink).await;
    Ok(())
}
