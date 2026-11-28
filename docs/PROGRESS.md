# SM Craft: project state (handoff notes)

Read this first after any interruption. Update it after every finished task.

## Architecture
- `core/` pure Rust (no GUI). Modules: paths, models, store, versions, mcjson (version JSON + loader merge),
  install (libs/assets/natives), java (detect/cache/Temurin download), launch (arg building), loaders (Fabric/Quilt),
  modrinth (search, deps, mods, FPS pack, .mrpack), engine (launch pipeline), diagnose, download (SHA-1/256/512).
- `src-tauri/` thin Tauri v2 command layer. Events: `progress`, `log`, `game-state`.
- `src/` React + TS + zustand. Pages: Home, Instances, Java, Settings. Modals: New instance (Custom / Modrinth modpacks),
  Edit, Mods (Installed / Browse).
- Data: `%APPDATA%\SoloCraft` (Windows), `~/.local/share/SoloCraft` (Linux). NOT the same as the old Gemini build
  (`SoloCraftLauncher`), which the user once ran by mistake.

## Done and confirmed on the user's Windows PC
- Vanilla launch, Fabric launch with FPS pack, mod install, Modrinth modpack install (Fabulously Optimized).

## Done, tested only here (fake servers / unit tests)
- Quilt, mod update/disable/remove, dependency resolution, mrpack safety checks, Java cache.

## Milestone 3 (this delivery)
- Microsoft sign-in (device code flow, msauth.rs), Verify button, token refresh before launch; needs the user's own
  approved Azure client ID (Settings). Chain tested only against a local fake server. NOT yet tested with real Microsoft.
- Folders block removed from details panel; "Open mods folder" lives in the Mods window. (Worlds/resourcepacks/etc.
  buttons failed on the user's PC for an undiagnosed reason; removed at his request.)
- Settings page is a grid of blocks.

## Milestone 4 (this delivery)
- Storage: pointer file `%APPDATA%\SoloCraft\location.json` -> data folder anywhere (core/src/storage.rs). First-run prompt,
  Settings -> Storage (drive list with free space, usage, Browse via tauri-plugin-dialog, verified move, restart needed),
  missing-drive fallback with warning. Tested with unit tests only (Windows drives/dialog untested on a real PC).
- Logs: per-instance "Log files" window (launcher.log kept on disk, latest.log, old logs incl. .gz, crash reports).
- Launch progress shows time left for the current step (frontend estimate from last 8 s of speed).
- User reported: changing 4-5 video/mod settings in his instance gave ~+200 FPS; asked which settings (unknown yet).
  Idea: "Recommended performance settings" button once we know which options.

- Microsoft sign-in no longer auto-opens the browser (user: Azure login logged him out of the whole browser). Still needs an approved Azure client ID; declined to borrow another launcher's ID.

## Pending (in priority order)
1. (done) Open-folder buttons added; FPS pack extended with Sodium Extra, More Culling, Cull Leaves.
2. FPS pack v2 (gap vs Fabulously Optimized: 200 vs 300-400 FPS at render/sim 12).
3. Per-instance Logs tab with saved log files; launch progress bar with time estimate.
4. Forge / NeoForge (needs installer step), then CurseForge (needs an API key; many packs forbid third-party download).
5. Import (.mrpack file, Prism/MultiMC), instance copy, drag-drop groups, Microsoft login, skins.
6. GUI polish pass (LabyMod-style), Linux packaging, Android (separate project), website later.

## Rules from the user
- He specifies goals; Claude decides details and builds. Everything explained in detail.
- Windows first, then Linux, then Android.
- After each finished task compress context; after an interruption recheck only the interrupted part.
- Windows build: set `$env:CARGO_TARGET_DIR="C:\sc-target"`; extract the zip into a fresh folder; close old app (port 1420).
- Every delivery message must include exact Windows run commands (extract, npm install, CARGO_TARGET_DIR, npm run tauri dev).
- Zip file times come from the sandbox clock (earlier than his local time), so Cargo can think core is unchanged and skip rebuilding it. Run commands must include the "touch" step (set LastWriteTime to now on core and src-tauri sources) after Expand-Archive.

## M5 (Forge/NeoForge + file import)
- core/src/forge.rs: version lists from maven-metadata.xml; installer jar run headless (`--installClient` in scratch dir with fake launcher_profiles.json + vanilla json/jar); result copied to our versions/ + libraries/. Profile ids: forge-<mc>-<lv>, neoforge-<lv>. Engine installs on first launch.
- install.rs: libs without URL (installer-made patched jars) join classpath if present on disk.
- core/src/import.rs: .mrpack/.zip (Modrinth), Prism/MultiMC zip (mmc-pack.json + .minecraft/), CurseForge detected -> clear "not supported" message. UI: New instance -> "Import file" tab.
- mrpack with forge/neoforge deps now accepted. FPS toggle only for Fabric/Quilt.
- Settings: Microsoft setup guide (details list). Still needs approved Azure app ID.
- Tested: unit + fake-installer jar (javac) + prism zip import. NOT tested on real Forge servers/real installer. NeoForge year-style MC (26.x) mapping is a guess.
- Not done: Doki-style in-game client (needs a Java/Fabric mod; separate milestone), CurseForge.

## M6 (SM Craft Client in-game mod)
- client-mod/: Fabric 1.21.4 Gradle project (Loom 1.9, yarn 1.21.4+build.8, fabric-api 0.119.2+1.21.4). Java: SoloClient (entry, Right Shift), Config (config/solocraft-client.json), Module/Modules (FPS, Coordinates, CPS, Keystrokes, Ping, Armor, Potion effects, Memory, Clock, Direction, Day counter, Server address, Toggle Sprint, Fullbright), Hud helpers, MenuScreen (Doki-style grid), HudEditorScreen (drag).
- NOT compiled in sandbox (no Maven/Mojang access): only syntax-checked with javac. First Windows build may need small API-name fixes -> user pastes the gradle error.
- Launcher: core/src/clientmod.rs (find jar in client-mod/build/libs or <data>/client/, install into mods + Fabric API), commands client_mod_status/install/remove, banner in Mods window.
- Built-in MS client id: env SOLOCRAFT_MS_CLIENT_ID at build time (msauth::builtin_client_id); Settings hides setup box when present. Approval from Mojang still required.
- Pending: Waypoints, Cosmetics, other MC versions for the mod, CurseForge.
- M6b: crash from user (Cull Leaves mixin failed, not our mod). FPS_STACK drops cull-leaves; diagnose.rs names the mod when a Mixin apply fails. User got JDK21 + gradle mirror (Tencent) working for client-mod build. Azure: user has only Gmail MSA, portal rejects (no directory); advised: someone with Azure free account registers app, shares client ID (not secret).
- M7: client-mod multi-version: versions/<mc>.properties + src/compat/{mc1201,mc1204,mc121}; build-all.ps1 -> dist/solocraft-client-<mc>-1.0.0.jar; versions 1.20.1, 1.20.4, 1.21.1, 1.21.4 (Fabric API coords for 1.20.4/1.21.1 are best recollection, unverified). HUD default layout auto-stacks (no overlap) until a module is moved. Launcher picks jar by instance MC version (clientmod.rs). Not compiled in sandbox.
- M7b: build-all.ps1 now auto-generates versions/<mc>.properties (Fabric meta yarn + Modrinth fabric-api lookup); supports 1.20..1.21.4 (12 versions, -Versions / -All). NEXT (needs user's confirmation that 1.21.4 works first): compat families for 1.21.5, 1.21.6-1.21.8 (JOML matrices), 1.21.9-1.21.11 (Click/KeyInput events, KeyBinding.Category), then 26.x (unobfuscated/Mojang names, new Loom). User's versions: he plays 1.21.11, friend 1.21.10, another friend 1.20.x, others 26.x.
