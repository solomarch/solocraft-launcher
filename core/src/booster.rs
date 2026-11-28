//! FPS Booster: looks at the PC, picks a performance tier, installs the FPS mods and writes
//! settings that suit that PC, so nobody has to open a config file. Everything it changes is
//! backed up first, and `undo` puts it back.

use crate::events::EventSink;
use crate::models::Instance;
use crate::modrinth::{self, FpsReport};
use crate::paths::Paths;
use crate::store::Store;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub ram_mb: u64,
    pub threads: usize,
    pub gpu: String,
    pub discrete_gpu: bool,
}

/// Read-only summary of the host used to explain Minecraft readiness.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemReport {
    pub os: String,
    pub architecture: String,
    pub processor: String,
    pub physical_cores: usize,
    pub graphics_drivers: String,
    pub hardware: Hardware,
    pub available_ram_mb: u64,
    pub free_disk_mb: Option<u64>,
    pub java: Vec<crate::models::JavaInstall>,
    pub diagnoses: Vec<String>,
}

pub fn system_scan(data_root: &Path) -> SystemReport {
    let hardware = detect();
    let available_ram_mb = available_ram_mb();
    let (processor, physical_cores) = processor_info();
    let graphics_drivers = if cfg!(windows) {
        run("powershell", &["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_VideoController | ForEach-Object { \"$($_.Name) | driver $($_.DriverVersion) | status $($_.ConfigManagerErrorCode)\" }) -join '; '"]).unwrap_or_default().trim().to_string()
    } else {
        run("sh", &["-c", "glxinfo -B 2>/dev/null | sed -n '/OpenGL vendor string:/p; /OpenGL renderer string:/p; /OpenGL version string:/p'"]).unwrap_or_default().trim().replace('\n', " · ")
    };
    let os = if cfg!(windows) {
        run("powershell", &["-NoProfile", "-NonInteractive", "-Command", "$o=Get-CimInstance Win32_OperatingSystem; \"$($o.Caption) (build $($o.BuildNumber))\""]).unwrap_or_else(|| "Windows (version unavailable)".into()).trim().to_string()
    } else if cfg!(target_os = "linux") {
        std::fs::read_to_string("/etc/os-release").ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))).unwrap_or_else(|| "Linux".into())
    } else { std::env::consts::OS.to_string() };
    let free_disk_mb = if cfg!(windows) {
        let root = data_root.to_string_lossy().replace('\'', "''");
        run("powershell", &["-NoProfile", "-Command", &format!("$p='{}'; $d=[IO.Path]::GetPathRoot($p); $id=$d.Substring(0,2); (Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='$id'\").FreeSpace", root)]).and_then(|s| s.trim().parse::<u64>().ok()).map(|b| b / 1024 / 1024)
    } else {
        run("df", &["-Pk", &data_root.to_string_lossy()]).and_then(|s| s.lines().last().and_then(|l| l.split_whitespace().nth(3)?.parse::<u64>().ok())).map(|kb| kb / 1024)
    };
    let java = Vec::new();
    let mut diagnoses = Vec::new();
    if available_ram_mb > 0 && available_ram_mb < 2048 { diagnoses.push("Available memory is below 2 GB. Close memory-heavy apps before launching Minecraft.".into()); }
    if hardware.ram_mb > 0 && hardware.ram_mb < 4096 { diagnoses.push("This device has less than 4 GB of RAM; use a small mod set and keep at least 1.5 GB for the operating system.".into()); }
    if hardware.gpu.is_empty() { diagnoses.push("Could not identify a graphics adapter. Update the graphics driver and check that OpenGL is available.".into()); }
    if cfg!(windows) { diagnose_windows_graphics(&graphics_drivers, !hardware.gpu.is_empty(), &mut diagnoses); }
    if let Some(free) = free_disk_mb { if free < 4096 { diagnoses.push("Less than 4 GB is free on the launcher drive. Free disk space before installing or updating game files.".into()); } }
    if diagnoses.is_empty() { diagnoses.push("No major hardware or storage issue was detected. Mod compatibility and graphics drivers can still affect launch performance.".into()); }
    SystemReport { os, architecture: std::env::consts::ARCH.into(), processor, physical_cores, graphics_drivers, hardware, available_ram_mb, free_disk_mb, java, diagnoses }
}

fn diagnose_windows_graphics(details: &str, gpu_detected: bool, diagnoses: &mut Vec<String>) {
    if details.split("; ").any(|g| g.ends_with("status 22") || g.ends_with("status 10")) {
        diagnoses.push("Windows reports that a graphics adapter is disabled or failed to start (Device Manager status 10/22). Enable it or reinstall its driver, then restart the PC.".into());
    }
    if gpu_detected && details.is_empty() {
        diagnoses.push("Windows found a graphics adapter but could not read its driver status through CIM. Check the adapter in Device Manager and install the manufacturer's driver if needed.".into());
    }
    if details.contains("driver ") && details.contains("driver  |") {
        diagnoses.push("Windows did not return a graphics driver version. Install the latest driver from your PC or GPU manufacturer.".into());
    }
}

fn processor_info() -> (String, usize) {
    if cfg!(windows) {
        let name = run("powershell", &["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)"]).unwrap_or_default().trim().to_string();
        let cores = run("powershell", &["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_Processor | Measure-Object -Property NumberOfCores -Sum).Sum"]).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        return (name, cores);
    }
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let name = info.lines().find_map(|l| l.strip_prefix("model name\t: ").map(str::to_owned)).unwrap_or_default();
    let cores = info.lines().find_map(|l| l.strip_prefix("cpu cores\t: ").and_then(|s| s.parse().ok())).unwrap_or(0);
    (name, cores)
}

fn available_ram_mb() -> u64 {
    if cfg!(windows) { return run("powershell", &["-NoProfile", "-Command", "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory"]).and_then(|s| s.trim().parse::<u64>().ok()).map(|kb| kb / 1024).unwrap_or(0); }
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("MemAvailable:").and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok()))).map(|kb| kb / 1024).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Potato,
    Balanced,
    Performance,
}

impl Tier {
    pub fn id(self) -> &'static str {
        match self {
            Tier::Potato => "potato",
            Tier::Balanced => "balanced",
            Tier::Performance => "performance",
        }
    }
    pub fn parse(s: &str) -> Option<Tier> {
        match s {
            "potato" => Some(Tier::Potato),
            "balanced" => Some(Tier::Balanced),
            "performance" => Some(Tier::Performance),
            _ => None,
        }
    }
}

/// Everything the booster would change, shown in the UI before anything is applied.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub tier: String,
    pub recommended: String,
    pub hardware: Hardware,
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub jvm_args: String,
    /// (setting, value) pairs written to options.txt
    pub options: Vec<(String, String)>,
    pub render_distance: u32,
    pub simulation_distance: u32,
    pub already_boosted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostReport {
    pub plan: Plan,
    pub mods: Option<FpsReport>,
    pub notes: Vec<String>,
}

// ---------- looking at the PC ----------

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn detect() -> Hardware {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut ram_mb = 0u64;
    let mut gpu = String::new();
    if cfg!(windows) {
        if let Some(o) = run(
            "powershell",
            &["-NoProfile", "-Command", "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory"],
        ) {
            ram_mb = o.trim().parse::<u64>().unwrap_or(0) / 1024 / 1024;
        }
        if let Some(o) = run(
            "powershell",
            &["-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }) -join ';'"],
        ) {
            gpu = o.trim().to_string();
        }
    } else {
        if let Ok(m) = std::fs::read_to_string("/proc/meminfo") {
            for l in m.lines() {
                if let Some(r) = l.strip_prefix("MemTotal:") {
                    let kb: u64 = r.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
                    ram_mb = kb / 1024;
                }
            }
        }
        if let Some(o) = run("lspci", &[]) {
            let names: Vec<String> = o
                .lines()
                .filter(|l| l.contains("VGA") || l.contains("3D controller"))
                .map(|l| l.splitn(3, ':').nth(2).unwrap_or(l).trim().to_string())
                .collect();
            gpu = names.join(";");
        }
    }
    let discrete_gpu = gpu_is_discrete(&gpu);
    Hardware { ram_mb, threads, gpu, discrete_gpu }
}

/// True when any listed GPU looks like a dedicated card (not Intel/AMD integrated graphics).
pub fn gpu_is_discrete(names: &str) -> bool {
    names.split(';').any(|n| {
        let n = n.to_lowercase();
        let nvidia = n.contains("nvidia") || n.contains("geforce") || n.contains("rtx") || n.contains("gtx") || n.contains("quadro");
        let amd = n.contains("radeon rx") || n.contains("radeon pro") || n.contains(" rx ") || n.contains("radeon vii");
        let arc = n.contains("intel") && n.contains("arc");
        nvidia || amd || arc
    })
}

/// Picks the tier that suits this PC.
pub fn recommend(hw: &Hardware) -> Tier {
    // ram_mb == 0 means detection failed: assume an average PC rather than the weakest.
    let ram = if hw.ram_mb == 0 { 8192 } else { hw.ram_mb };
    if ram < 6000 || hw.threads < 4 || (!hw.discrete_gpu && ram < 12000) {
        Tier::Potato
    } else if hw.discrete_gpu && ram >= 12000 && hw.threads >= 8 {
        Tier::Performance
    } else {
        Tier::Balanced
    }
}

// ---------- what each tier sets ----------

pub fn make_plan(hw: &Hardware, tier: Tier, already_boosted: bool) -> Plan {
    let ram = if hw.ram_mb == 0 { 8192 } else { hw.ram_mb };
    let (max_ram, render, sim, entity, particles, graphics, clouds, ao, blend, mip, shadows, fps): (u64, u32, u32, &str, u32, u32, &str, bool, u32, u32, bool, u32) = match tier {
        Tier::Potato => ((ram * 2 / 5).clamp(1536, 3072), 12, 12, "0.5", 2, 0, "false", false, 1, 0, false, 120),
        Tier::Balanced => ((ram / 4).clamp(3072, 6144), 12, 12, "0.75", 1, 1, "false", true, 3, 2, true, 144),
        Tier::Performance => ((ram / 4).clamp(4096, 8192), 14, 12, "1.0", 1, 1, "false", true, 5, 4, true, 260),
    };
    // never give the game more than 60% of the RAM
    let max_ram = max_ram.min(ram * 3 / 5) as u32;
    let min_ram = if tier == Tier::Potato { 512 } else { 1024 };
    let mut jvm = String::from(
        "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=80 \
         -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1ReservePercent=20 \
         -XX:InitiatingHeapOccupancyPercent=15",
    );
    if max_ram >= 4096 {
        jvm.push_str(" -XX:G1HeapRegionSize=8M");
    }
    let b = |v: bool| if v { "true" } else { "false" }.to_string();
    let options = vec![
        ("renderDistance".to_string(), render.to_string()),
        ("simulationDistance".to_string(), sim.to_string()),
        ("entityDistanceScaling".to_string(), entity.to_string()),
        ("particles".to_string(), particles.to_string()),
        ("graphicsMode".to_string(), graphics.to_string()),
        ("renderClouds".to_string(), format!("\"{clouds}\"")),
        ("ao".to_string(), b(ao)),
        ("biomeBlendRadius".to_string(), blend.to_string()),
        ("mipmapLevels".to_string(), mip.to_string()),
        ("entityShadows".to_string(), b(shadows)),
        ("enableVsync".to_string(), "false".to_string()),
        ("maxFps".to_string(), fps.to_string()),
    ];
    Plan {
        tier: tier.id().into(),
        recommended: recommend(hw).id().into(),
        hardware: hw.clone(),
        min_ram_mb: min_ram,
        max_ram_mb: max_ram.max(min_ram),
        jvm_args: jvm,
        options,
        render_distance: render,
        simulation_distance: sim,
        already_boosted,
    }
}

/// Sodium's settings for a tier, merged into the file it already has.
fn sodium_patch(tier: Tier, hw: &Hardware) -> serde_json::Value {
    let fast = tier == Tier::Potato;
    let threads = if tier == Tier::Performance { hw.threads.saturating_sub(2).clamp(1, 12) } else { 0 };
    serde_json::json!({
        "quality": {
            "weather_quality": if fast { "FAST" } else { "DEFAULT" },
            "leaves_quality": if fast { "FAST" } else { "DEFAULT" }
        },
        "performance": {
            "chunk_builder_threads": threads,
            "always_defer_chunk_updates_v2": true,
            "animate_only_visible_textures": true,
            "use_entity_culling": true,
            "use_fog_occlusion": true,
            "use_block_face_culling": true,
            "use_no_error_g_l_context": true
        }
    })
}

fn merge_json(base: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(b), Some(p)) = (base.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            match b.get_mut(k) {
                Some(bv) if bv.is_object() && v.is_object() => merge_json(bv, v),
                _ => {
                    b.insert(k.clone(), v.clone());
                }
            }
        }
    } else {
        *base = patch.clone();
    }
}

/// Sets `key:value` lines in an options.txt text, keeping every other line.
pub fn patch_options(text: &str, set: &[(String, String)]) -> String {
    let mut done = vec![false; set.len()];
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let key = line.split(':').next().unwrap_or("");
        if let Some(i) = set.iter().position(|(k, _)| k == key) {
            out.push(format!("{}:{}", set[i].0, set[i].1));
            done[i] = true;
        } else {
            out.push(line.to_string());
        }
    }
    for (i, (k, v)) in set.iter().enumerate() {
        if !done[i] {
            out.push(format!("{k}:{v}"));
        }
    }
    out.join("\n") + "\n"
}

// ---------- backup / undo ----------

#[derive(Debug, Serialize, Deserialize, Default)]
struct Backup {
    min_ram_mb: u32,
    max_ram_mb: u32,
    jvm_args: String,
    /// path inside .minecraft -> old content (None = the file did not exist)
    files: BTreeMap<String, Option<String>>,
    installed_mods: Vec<String>,
}

fn backup_path(paths: &Paths, id: &str) -> std::path::PathBuf {
    paths.instance_dir(id).join("booster-backup.json")
}

fn load_backup(paths: &Paths, id: &str) -> Option<Backup> {
    serde_json::from_str(&std::fs::read_to_string(backup_path(paths, id)).ok()?).ok()
}

fn remember_file(b: &mut Backup, game: &Path, rel: &str) -> Result<()> {
    if !b.files.contains_key(rel) {
        let path = game.join(rel);
        let old = match std::fs::read_to_string(&path) {
            Ok(value) => Some(value),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("backing up {}", path.display())),
        };
        b.files.insert(rel.to_string(), old);
    }
    Ok(())
}

pub fn is_boosted(paths: &Paths, id: &str) -> bool {
    backup_path(paths, id).exists()
}

pub fn plan_for(paths: &Paths, inst: &Instance, tier: Option<&str>) -> Plan {
    let hw = detect();
    let t = tier.and_then(Tier::parse).unwrap_or_else(|| recommend(&hw));
    make_plan(&hw, t, is_boosted(paths, &inst.id))
}

pub async fn apply(
    paths: &Paths,
    store: &Store,
    inst: &Instance,
    tier: Option<&str>,
    sink: &dyn EventSink,
) -> Result<BoostReport> {
    let hw = detect();
    let t = tier.and_then(Tier::parse).unwrap_or_else(|| recommend(&hw));
    let plan = make_plan(&hw, t, is_boosted(paths, &inst.id));
    let game = paths.game_dir(&inst.id);
    std::fs::create_dir_all(&game)?;
    let mut notes = vec![];

    // 1. the mods (Fabric / Quilt only)
    let mut backup = load_backup(paths, &inst.id).unwrap_or_else(|| Backup {
        min_ram_mb: inst.min_ram_mb,
        max_ram_mb: inst.max_ram_mb,
        jvm_args: inst.jvm_args.clone(),
        ..Default::default()
    });
    let mods = if modrinth::loader_filter(&inst.loader).is_empty() {
        notes.push(format!(
            "The FPS mods need a Fabric or Quilt instance (this one is {}). Memory and game settings were still tuned.",
            inst.loader
        ));
        None
    } else {
        let r = modrinth::install_fps_stack(paths, inst, sink).await?;
        for m in &r.installed {
            backup.installed_mods.push(m.filename.clone());
        }
        if !r.skipped.is_empty() {
            notes.push(format!("No build for Minecraft {} yet: {}.", inst.mc_version, r.skipped.join(", ")));
        }
        Some(r)
    };

    // 2. game settings
    remember_file(&mut backup, &game, "options.txt")?;
    let old = std::fs::read_to_string(game.join("options.txt")).unwrap_or_default();
    std::fs::write(game.join("options.txt"), patch_options(&old, &plan.options)).context("writing options.txt")?;

    // 3. Sodium (only when it is installed)
    let sodium_installed = mods
        .as_ref()
        .map(|r| r.installed.iter().any(|m| m.project_id == "AANobbMI" || m.title.to_lowercase() == "sodium"))
        .unwrap_or(false)
        || std::fs::read_dir(game.join("mods"))
            .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("sodium-fabric")))
            .unwrap_or(false);
    if sodium_installed {
        let rel = "config/sodium-options.json";
        remember_file(&mut backup, &game, rel)?;
        let mut cur: serde_json::Value = std::fs::read_to_string(game.join(rel))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        merge_json(&mut cur, &sodium_patch(t, &hw));
        std::fs::create_dir_all(game.join("config"))?;
        std::fs::write(game.join(rel), serde_json::to_string_pretty(&cur)?)?;
    }

    // 4. memory and JVM flags
    std::fs::write(backup_path(paths, &inst.id), serde_json::to_string_pretty(&backup)?)?;
    let mut updated = inst.clone();
    updated.min_ram_mb = plan.min_ram_mb;
    updated.max_ram_mb = plan.max_ram_mb;
    updated.jvm_args = plan.jvm_args.clone();
    store.save_instance(&updated)?;
    Ok(BoostReport { plan, mods, notes })
}

pub fn undo(paths: &Paths, store: &Store, inst: &Instance) -> Result<()> {
    let b = load_backup(paths, &inst.id).ok_or_else(|| anyhow!("Nothing to undo: the FPS Booster was not applied to this instance."))?;
    let game = paths.game_dir(&inst.id);
    for (rel, content) in &b.files {
        let p = game.join(rel);
        match content {
            Some(c) => std::fs::write(&p, c)?,
            None => {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    for f in &b.installed_mods {
        let _ = modrinth::remove_mod(paths, &inst.id, f);
    }
    let mut updated = inst.clone();
    updated.min_ram_mb = b.min_ram_mb;
    updated.max_ram_mb = b.max_ram_mb;
    updated.jvm_args = b.jvm_args.clone();
    store.save_instance(&updated)?;
    std::fs::remove_file(backup_path(paths, &inst.id))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(ram: u64, threads: usize, gpu: &str) -> Hardware {
        Hardware { ram_mb: ram, threads, gpu: gpu.into(), discrete_gpu: gpu_is_discrete(gpu) }
    }

    #[test]
    fn tiers() {
        assert_eq!(recommend(&hw(32000, 12, "NVIDIA GeForce RTX 4050 Laptop GPU;AMD Radeon(TM) Graphics")), Tier::Performance);
        assert_eq!(recommend(&hw(16000, 6, "NVIDIA GeForce GTX 1650")), Tier::Balanced);
        assert_eq!(recommend(&hw(8000, 4, "Intel(R) UHD Graphics")), Tier::Potato);
        assert_eq!(recommend(&hw(4000, 8, "NVIDIA GeForce RTX 3060")), Tier::Potato);
        // RAM unknown (0) is treated as 8 GB: with no dedicated GPU that is the Potato tier
        assert_eq!(recommend(&hw(0, 8, "")), Tier::Potato);
    }

    #[test]
    fn discrete_detection() {
        assert!(gpu_is_discrete("Intel(R) UHD Graphics;NVIDIA GeForce RTX 4050"));
        assert!(!gpu_is_discrete("AMD Radeon(TM) Graphics"));
        assert!(gpu_is_discrete("AMD Radeon RX 6600"));
        assert!(!gpu_is_discrete(""));
    }

    #[test]
    fn plan_never_takes_too_much_ram() {
        let p = make_plan(&hw(8000, 8, "NVIDIA GeForce RTX 3060"), Tier::Performance, false);
        assert!(p.max_ram_mb as u64 <= 8000 * 3 / 5);
        let p = make_plan(&hw(32000, 12, "NVIDIA GeForce RTX 4050"), Tier::Performance, false);
        assert_eq!(p.max_ram_mb, 8000.min(8192));
        assert!(p.jvm_args.contains("G1HeapRegionSize"));
    }

    #[test]
    fn options_are_patched_not_replaced() {
        let old = "version:3955\nrenderDistance:32\nfov:0.5\nmaxFps:60\n";
        let new = patch_options(old, &[("renderDistance".into(), "12".into()), ("particles".into(), "1".into())]);
        assert!(new.contains("renderDistance:12"));
        assert!(new.contains("fov:0.5"));
        assert!(new.contains("maxFps:60"));
        assert!(new.contains("particles:1"));
        assert!(!new.contains("renderDistance:32"));
    }

    #[test]
    fn sodium_json_merges() {
        let mut cur = serde_json::json!({"quality":{"enable_vignette":false,"leaves_quality":"FANCY"},"advanced":{"x":1}});
        merge_json(&mut cur, &sodium_patch(Tier::Balanced, &hw(16000, 8, "")));
        assert_eq!(cur["quality"]["enable_vignette"], false);
        assert_eq!(cur["quality"]["leaves_quality"], "DEFAULT");
        assert_eq!(cur["advanced"]["x"], 1);
        assert_eq!(cur["performance"]["use_block_face_culling"], true);
    }

    #[test]
    fn windows_graphics_failure_is_explained() {
        let mut findings = Vec::new();
        diagnose_windows_graphics("Intel UHD | driver 31.0 | status 22", true, &mut findings);
        assert!(findings.iter().any(|d| d.contains("Device Manager status 10/22")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_scan_reads_windows_cim_data() {
        let report = system_scan(&std::env::temp_dir());
        assert!(report.os.to_lowercase().contains("windows"));
        assert!(!report.processor.is_empty());
        assert!(report.hardware.ram_mb > 0);
    }
}
