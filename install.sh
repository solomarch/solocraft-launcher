#!/usr/bin/env bash
# SoloCraft Launcher - install or update (Linux, x86_64)
#   curl -fsSL https://raw.githubusercontent.com/solomarch/solocraft-launcher/main/install.sh | bash
# Running it again updates to the newest release. SOLOCRAFT_FORCE=1 reinstalls.
set -euo pipefail

REPO="${SOLOCRAFT_REPO:-solomarch/solocraft-launcher}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
INSTALL_DIR="$DATA_HOME/solocraft-launcher"
BIN_DIR="$HOME/.local/bin"
CLIENT_DIR="$DATA_HOME/SoloCraft/client"   # where the launcher looks for the in-game mod jars
APPIMAGE="$INSTALL_DIR/SoloCraft-Launcher.AppImage"

command -v curl >/dev/null 2>&1 || { echo "Please install curl first."; exit 1; }
[ "$(uname -m)" = "x86_64" ] || { echo "Only x86_64 builds are published for now (this is $(uname -m))."; exit 1; }

echo "SoloCraft Launcher - looking for the latest version ..."
json="$(curl -fsSL -H "User-Agent: solocraft-installer" "https://api.github.com/repos/$REPO/releases/latest")" || {
  echo "Could not read the latest release of $REPO. Is the repository public, and has a release been published yet?"
  exit 1
}
tag="$(printf '%s' "$json" | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
urls="$(printf '%s' "$json" | grep -o '"browser_download_url": *"[^"]*"' | sed -E 's/.*"(https[^"]*)"/\1/')"

mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$CLIENT_DIR" "$DATA_HOME/applications"
have=""
[ -f "$INSTALL_DIR/version.txt" ] && have="$(cat "$INSTALL_DIR/version.txt")"

if [ "$have" = "$tag" ] && [ -x "$APPIMAGE" ] && [ -z "${SOLOCRAFT_FORCE:-}" ]; then
  echo "SoloCraft Launcher is already up to date ($tag)."
else
  if pgrep -x solocraft-launcher >/dev/null 2>&1; then
    echo "SoloCraft Launcher is running. Close it (and any game started from it), then run this command again."
    exit 1
  fi
  app_url="$(printf '%s\n' "$urls" | grep -i '\.AppImage$' | head -n1 || true)"
  [ -n "$app_url" ] || { echo "Release $tag has no Linux build yet (it may still be building). Try again in a few minutes."; exit 1; }
  echo "Downloading $(basename "$app_url") ($tag) ..."
  curl -fL --progress-bar -o "$APPIMAGE.part" "$app_url"
  mv -f "$APPIMAGE.part" "$APPIMAGE"
  chmod +x "$APPIMAGE"
  echo "$tag" > "$INSTALL_DIR/version.txt"

  ln -sf "$APPIMAGE" "$BIN_DIR/solocraft"
  cat > "$DATA_HOME/applications/solocraft-launcher.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=SoloCraft Launcher
Comment=Minecraft launcher
Exec=env APPIMAGE_EXTRACT_AND_RUN=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 "$APPIMAGE"
Icon=applications-games
Terminal=false
Categories=Game;
DESK
  echo "Installed SoloCraft Launcher $tag."
fi

# In-game menu mod: one small jar per Minecraft version, picked automatically by the launcher.
count=0
for u in $(printf '%s\n' "$urls" | grep '/solocraft-client-.*\.jar$' || true); do
  f="$CLIENT_DIR/$(basename "$u")"
  if [ ! -s "$f" ]; then
    curl -fsSL -o "$f" "$u" && count=$((count + 1))
  fi
done
echo "SoloCraft Client mod files: $(ls "$CLIENT_DIR" 2>/dev/null | grep -c '\.jar$' || true) available ($count new)."

case ":$PATH:" in *":$BIN_DIR:"*) ;; *) echo "Note: add $BIN_DIR to your PATH to start it by typing 'solocraft'." ;; esac
echo "Done. Start it from your app menu ('SoloCraft Launcher') or run: solocraft"
echo "If it does not open: run  APPIMAGE_EXTRACT_AND_RUN=1 $APPIMAGE  to see the error (Arch/Manjaro: sudo pacman -S fuse2 may help)."
