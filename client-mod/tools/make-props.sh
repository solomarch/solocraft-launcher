#!/usr/bin/env bash
# Creates versions/<mc>.properties for one Minecraft version (CI helper; build-all.ps1 does this on Windows).
# Run inside client-mod/:   bash tools/make-props.sh 1.21.4
set -euo pipefail
v="$1"
mkdir -p versions
f="versions/$v.properties"
[ -f "$f" ] && exit 0
ge() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$2" ]; }   # $1 >= $2
if ge "$v" "1.21.5"; then echo "Minecraft $v is not supported yet"; exit 1; fi
if   ge "$v" "1.20.5"; then compat=mc121;  java=21
elif ge "$v" "1.20.2"; then compat=mc1204; java=17
else                        compat=mc1201; java=17; fi
yarn="$(curl -fsSL -A solocraft-ci "https://meta.fabricmc.net/v2/versions/yarn/$v" | jq -r '.[0].version')"
fab="$(curl -fsSL -A solocraft-ci -G "https://api.modrinth.com/v2/project/fabric-api/version" \
        --data-urlencode "game_versions=[\"$v\"]" --data-urlencode 'loaders=["fabric"]' | jq -r '.[0].version_number')"
[ -n "$yarn" ] && [ "$yarn" != "null" ] && [ -n "$fab" ] && [ "$fab" != "null" ] || { echo "No mappings / Fabric API found for $v"; exit 1; }
printf 'yarn=%s\nfabric=%s\nloader=0.16.10\ncompat=%s\njava=%s\nmcrange=%s\n' "$yarn" "$fab" "$compat" "$java" "$v" > "$f"
cat "$f"
