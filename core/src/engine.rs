use crate::diagnose::diagnose;
use crate::events::{emit_progress, emit_state, EventSink};
use crate::models::LogLine;
use crate::paths::Paths;
use crate::store::Store;
use crate::{install, java, launch, loaders};
use anyhow::{anyhow, Result};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::oneshot;

const TAIL_LINES: usize = 600;

pub struct Engine {
    pub store: Store,
    running: Mutex<HashMap<String, Option<oneshot::Sender<()>>>>,
}

fn classify(line: &str, from_stderr: bool) -> &'static str {
    if line.contains("/FATAL]")
        || line.contains("/ERROR]")
        || line.starts_with("Exception")
        || line.starts_with("Caused by")
        || line.starts_with("\tat ")
        || line.contains("Exception in thread")
    {
        "error"
    } else if line.contains("/WARN]") || line.starts_with("WARNING") {
        "warn"
    } else if from_stderr {
        "warn"
    } else {
        "info"
    }
}

async fn pump<R: AsyncRead + Unpin>(
    reader: R,
    sink: Arc<dyn EventSink>,
    id: String,
    tail: Arc<Mutex<VecDeque<String>>>,
    from_stderr: bool,
) {
    let mut r = BufReader::new(reader);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buf).trim_end().to_string();
                {
                    let mut t = tail.lock().unwrap();
                    if t.len() >= TAIL_LINES {
                        t.pop_front();
                    }
                    t.push_back(text.clone());
                }
                sink.log(LogLine {
                    instance_id: id.clone(),
                    level: classify(&text, from_stderr).to_string(),
                    text,
                });
            }
        }
    }
}

fn log_info(sink: &dyn EventSink, id: &str, text: String) {
    sink.log(LogLine {
        instance_id: id.to_string(),
        level: "info".into(),
        text,
    });
}

impl Engine {
    pub fn new(root: PathBuf) -> Result<Arc<Engine>> {
        let paths = Paths::new(root);
        paths.ensure_base()?;
        Ok(Arc::new(Engine {
            store: Store::new(paths),
            running: Mutex::new(HashMap::new()),
        }))
    }

    pub fn paths(&self) -> &Paths {
        &self.store.paths
    }

    pub fn running_ids(&self) -> Vec<String> {
        self.running.lock().unwrap().keys().cloned().collect()
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.running.lock().unwrap().contains_key(id)
    }

    /// Stops a running game. Returns false when the instance is not running.
    pub fn kill(&self, id: &str) -> bool {
        let mut r = self.running.lock().unwrap();
        match r.get_mut(id) {
            Some(slot) => {
                if let Some(tx) = slot.take() {
                    let _ = tx.send(());
                }
                true
            }
            None => false,
        }
    }

    /// Runs the full pipeline for one instance and returns when the game exits.
    /// All progress and failures are reported through the sink.
    pub async fn launch(self: &Arc<Self>, id: &str, sink: Arc<dyn EventSink>) {
        let (tx, rx) = oneshot::channel();
        {
            let mut r = self.running.lock().unwrap();
            if r.contains_key(id) {
                drop(r);
                emit_state(&*sink, id, "error", "This instance is already running");
                return;
            }
            r.insert(id.to_string(), Some(tx));
        }
        // keep a copy of everything the launcher prints in launcher.log
        let sink: Arc<dyn EventSink> = Arc::new(crate::logs::TeeSink::new(sink, &self.store.paths, id));
        emit_state(&*sink, id, "preparing", "Preparing");
        let res = self.run_pipeline(id, sink.clone(), rx).await;
        self.running.lock().unwrap().remove(id);
        if let Err(e) = res {
            emit_state(&*sink, id, "error", &format!("{e:#}"));
        }
    }

    async fn run_pipeline(
        &self,
        id: &str,
        sink: Arc<dyn EventSink>,
        mut kill_rx: oneshot::Receiver<()>,
    ) -> Result<()> {
        let paths = self.store.paths.clone();
        let inst = self.store.get_instance(id)?;
        let account = self
            .store
            .resolve_account(&inst)
            .ok_or_else(|| anyhow!("Add an account in Settings before launching"))?;

        if matches!(inst.loader.as_str(), "forge" | "neoforge") {
            let lv = if inst.loader_version.trim().is_empty() {
                loaders::latest_loader_version(&inst.loader, &inst.mc_version).await?
            } else {
                inst.loader_version.clone()
            };
            let pid = loaders::profile_id(&inst.loader, &lv, &inst.mc_version);
            if !paths.version_json(&pid).exists() {
                emit_state(&*sink, id, "preparing", "Setting up the mod loader");
                let vanilla = crate::versions::load_version(&paths, &inst.mc_version).await?;
                let j = java::resolve_java(&paths, &inst, java::required_major(&vanilla), sink.clone(), id).await?;
                crate::forge::ensure_installed(&paths, &inst.loader, &inst.mc_version, &lv, &j.path, &*sink, id).await?;
            }
        }
        emit_state(&*sink, id, "preparing", "Reading version data");
        let (version, loader_ver) = loaders::load_effective_version(&paths, &inst).await?;
        if !loader_ver.is_empty() {
            log_info(&*sink, id, format!("[SoloCraft] Loader: {} {}", inst.loader, loader_ver));
        }
        let major = java::required_major(&version);

        emit_state(&*sink, id, "preparing", &format!("Finding Java {major}"));
        let java = java::resolve_java(&paths, &inst, major, sink.clone(), id).await?;
        log_info(
            &*sink,
            id,
            format!("[SoloCraft] Using Java {} ({}) at {}", java.version, java.vendor, java.path),
        );

        emit_state(&*sink, id, "downloading", "Downloading game files");
        let game_dir = paths.game_dir(id);
        tokio::fs::create_dir_all(&game_dir).await?;
        let plan = install::install_version(&paths, &version, &game_dir, &*sink, id).await?;

        emit_progress(&*sink, id, "Preparing natives", 0, 0);
        let natives_dir = paths.natives_dir(id);
        {
            let n = plan.natives.clone();
            let d = natives_dir.clone();
            tokio::task::spawn_blocking(move || install::extract_natives(&n, &d)).await??;
        }

        let mut account = account;
        let mut mc_token: Option<String> = None;
        if account.kind == "microsoft" {
            emit_state(&*sink, id, "preparing", "Refreshing Microsoft sign-in");
            let (tok, session) = crate::msauth::ensure_token(&self.store, &account).await?;
            account.username = session.username;
            mc_token = Some(tok);
        }
        let built = launch::build(&paths, &inst, &version, &plan, &account, mc_token.as_deref(), &natives_dir, &game_dir);
        if inst.use_nvidia_gpu {
            launch::set_gpu_preference(&java.path);
        }

        emit_state(&*sink, id, "launching", "Starting Minecraft");
        log_info(
            &*sink,
            id,
            format!("[SoloCraft] Starting {} as {}", version.id, account.username),
        );
        let mut cmd = tokio::process::Command::new(&java.path);
        cmd.args(&built.args)
            .current_dir(&game_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in &built.env {
            cmd.env(k, v);
        }
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("Could not start Java at {}: {e}", java.path))?;

        let tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let h_out = child.stdout.take().map(|s| {
            tokio::spawn(pump(s, sink.clone(), id.to_string(), tail.clone(), false))
        });
        let h_err = child.stderr.take().map(|s| {
            tokio::spawn(pump(s, sink.clone(), id.to_string(), tail.clone(), true))
        });

        emit_state(&*sink, id, "running", "Minecraft is running");
        let started = std::time::Instant::now();
        let mut killed = false;
        let status = tokio::select! {
            s = child.wait() => s?,
            r = &mut kill_rx => {
                if r.is_ok() {
                    killed = true;
                    let _ = child.kill().await;
                }
                child.wait().await?
            }
        };
        if let Some(h) = h_out {
            let _ = h.await;
        }
        if let Some(h) = h_err {
            let _ = h.await;
        }

        // Record play time against the latest saved copy of the instance.
        if let Ok(mut fresh) = self.store.get_instance(id) {
            fresh.last_played = Some(chrono::Utc::now().timestamp());
            fresh.play_time_secs += started.elapsed().as_secs();
            let _ = self.store.save_instance(&fresh);
        }

        let code = status.code();
        if killed {
            emit_state(&*sink, id, "stopped", "Stopped");
        } else if status.success() {
            emit_state(&*sink, id, "stopped", "Game closed");
        } else {
            let text = tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join("\n");
            let msg = diagnose(&text).unwrap_or_else(|| {
                format!(
                    "The game exited with code {}. Open the console for details.",
                    code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into())
                )
            });
            emit_state(&*sink, id, "crashed", &msg);
        }
        Ok(())
    }
}
