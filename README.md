# SM Craft

A custom launcher for **Minecraft: Java Edition**. It combines the instance management of Prism/PolyMC,
one-click performance presets and an in-game menu (HUD) in a single, fast desktop app.

> **Status: early software.** Built and tested on Windows by its author. The Linux build is new and less tested.
> Expect rough edges and please [open an issue](../../issues) when something breaks.

## Install

Run one command. **Running the same command again updates to the newest version.**

**Windows – PowerShell**

```powershell
irm https://raw.githubusercontent.com/solomarch/sm-craft/main/install.ps1 | iex
```

**Windows – Command Prompt (cmd)**

```bat
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/solomarch/sm-craft/main/install.ps1 | iex"
```

**Linux (x86_64)**

```bash
curl -fsSL https://raw.githubusercontent.com/solomarch/sm-craft/main/install.sh | bash
```

### Uninstall

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/solomarch/sm-craft/main/uninstall.ps1 | iex
```

Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/solomarch/sm-craft/main/uninstall.sh | bash
```

Your instances, worlds and accounts are kept. Set `SOLOCRAFT_PURGE=1` first to delete them as well.


What the commands do:

- Windows: download the newest installer from the [Releases](../../releases) page and run it silently
  (per user, no administrator rights needed). Windows SmartScreen may warn about an unsigned app: choose
  *More info → Run anyway* if you trust this repository.
- Linux: download the newest AppImage into `~/.local/share/solocraft-launcher`, add a menu entry and a
  `solocraft` command in `~/.local/bin`.
- Both: copy the in-game menu mod files (see below) next to your game data. Your instances, worlds and
  settings are **never touched** by an update.

To remove it: uninstall *SM Craft* from Windows Settings → Apps, or on Linux delete
`~/.local/share/solocraft-launcher`, `~/.local/bin/solocraft` and
`~/.local/share/applications/solocraft-launcher.desktop`. Game data lives in `%APPDATA%\SoloCraft`
(Windows) or `~/.local/share/SoloCraft` (Linux) unless you moved it in Settings.

## Features

- **Instances and groups** with per-instance Java, memory, JVM arguments and live console/log files
- **Loaders:** Vanilla, Fabric, Quilt, Forge, NeoForge (Forge/NeoForge run the official installer on first launch)
- **Modrinth** search and install for mods and modpacks (checksums verified), mod updates, enable/disable/remove
- **Import** `.mrpack`, Modrinth `.zip` and Prism/MultiMC instance zips
- **FPS pack:** Sodium, Lithium, FerriteCore, ModernFix, ImmediatelyFast, Entity Culling and friends in one click
- **Java handling:** finds installed Java or downloads the right one for each Minecraft version
- **Storage:** keep all game data on any drive (Settings → Storage), moved with a verified copy
- **Server hosting:** start a separate Vanilla or Fabric server per instance, with Online/Offline authentication, port and player settings
- **Launch progress** with a time estimate, and plain-language crash explanations
- **SM Craft Client** (in-game menu): see below

## SM Craft Client – in-game menu

A small Fabric mod that opens an in-game menu with **Right Shift**: toggles for FPS, CPS, keystrokes,
coordinates, ping, armor status, potion effects, memory, clock, direction, day counter and server address,
plus Toggle Sprint and Fullbright. *Edit HUD Layout* lets you drag every element where you want it.

- Install per instance: open the instance → **Mods** → **SM Craft Client → Install**
  (Fabric instances, Minecraft 1.20 – 1.21.11).
- It needs Fabric API; the launcher installs that for you.
- Newer Minecraft versions (1.21.5+, 26.x) are planned.

## Accounts

- **Offline accounts** work out of the box.
- **Microsoft (premium) sign-in** uses the device-code flow: the launcher shows a code, you type it on
  Microsoft's own page. SM Craft never sees your password; sign-in tokens stay in a file on your own computer
  and are not sent anywhere except to Microsoft/Xbox/Minecraft services.
  Mojang must approve each launcher's Microsoft app before its sign-in works. Until that approval is granted
  you will see *"Minecraft services refused this app ID (403)"*. Maintainers: see below.

## Build from source

You need Node.js 20+, Rust (stable) and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/solomarch/sm-craft.git
cd solocraft-launcher
npm install
npm run tauri dev        # run in development
npm run tauri build      # make an installer
cargo test -p solocraft-core   # backend tests
```

Layout: `core/` pure Rust library (versions, downloads, Java, loaders, Modrinth, accounts, storage) ·
`src-tauri/` thin command layer · `src/` React + TypeScript interface · `client-mod/` the Fabric mod ·
`docs/PROGRESS.md` development notes.

The mod is built per Minecraft version: `cd client-mod && ./gradlew build -Pmc=1.21.4` after
`bash tools/make-props.sh 1.21.4` (Windows: `.\build-all.ps1 -Versions 1.21.4`).

## For maintainers: publishing an update

1. Change `"version"` in `src-tauri/tauri.conf.json` (and `package.json`) to e.g. `1.0.1`.
2. Commit and push, then tag and push the tag:
   ```bash
   git tag v1.0.1
   git push origin v1.0.1
   ```
3. GitHub Actions builds the Windows installer, the Linux AppImage and the mod jars and attaches them to a
   new release (about 15–30 minutes). Players get it by running the install command again.
4. When Mojang approves your Azure app: add a repository **variable** named `MS_CLIENT_ID` with the
   Application (client) ID (Settings → Secrets and variables → Actions → Variables) and publish a new
   release. From then on players sign in with just a code and never see Azure. (The ID is not a secret.)

## Not affiliated

SM Craft is an independent project and is not affiliated with or endorsed by Mojang or Microsoft.
Minecraft is a trademark of Mojang Synergies AB.


## New in this version
- **Keybind Manager** (in-game menu, Right Shift -> Keybinds): every key of the game and all mods on a clickable keyboard or in a list, conflicts in red, "Fix conflicts" in one click, swap on conflict.
- **FPS Booster** (launcher, Mods window -> FPS Booster): detects your PC, picks Potato / Balanced / Performance+, installs 9 FPS mods and writes memory, JVM and game settings (Sodium is tuned too). Undo restores everything.
- **One client jar for every Minecraft 1.21.x.**
