#!/usr/bin/env bash
# SM Craft - uninstall (Linux)
#   curl -fsSL https://raw.githubusercontent.com/solomarch/sm-craft/main/uninstall.sh | bash
# Your instances, worlds and accounts are KEPT. To delete them as well:  SOLOCRAFT_PURGE=1 bash uninstall.sh
set -u
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
pkill -x solocraft-launcher >/dev/null 2>&1 || true
rm -rf "$DATA_HOME/solocraft-launcher"
rm -f "$HOME/.local/bin/solocraft" "$DATA_HOME/applications/solocraft-launcher.desktop"
echo "SM Craft removed."
if [ -n "${SOLOCRAFT_PURGE:-}" ]; then
  rm -rf "$DATA_HOME/SoloCraft" "$HOME/.config/SoloCraft" "$HOME/.config/dev.solocraft.launcher"
  echo "All SM Craft data deleted."
else
  echo "Your instances and accounts were kept (run with SOLOCRAFT_PURGE=1 to delete them too)."
fi
