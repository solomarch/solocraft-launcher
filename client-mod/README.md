# SM Craft Client (Fabric mod)

In-game menu and HUD for SM Craft. Press **Right Shift** in game.

Supported (Fabric): **one jar for every Minecraft 1.21.x (1.21 up to 1.21.11)**, plus separate jars for 1.20 - 1.20.6.
Not yet: 26.x.

The in-game menu (Right Shift) is translucent over the blurred world. See [FEATURES.md](FEATURES.md) for implemented modules and the remaining safe feature plan.

FPS Companion Max is integrated into the SM Craft Client menu: Performance includes an opt-in 12+ chunk Max FPS Profile, a rolling 10-second average/1% low/frame-time HUD, and detected optimization-mod reporting. These run inside the SM Craft Client jar; the separate FPS Companion Max jar is not needed alongside it.

## Build (needs JDK 21 active; internet for the first build)

    .\build-all.ps1          builds the ONE jar for all 1.21.x into .\dist  (PowerShell)
    .\build-all.ps1 -Versions 1.20.1,1.20.4    older versions, one jar each

SM Craft finds the jars in `client-mod\dist` by itself
(Mods window -> SM Craft Client -> Install). If a version fails to build, the script
carries on with the others and lists what failed at the end.

## Other versions
`.\build-all.ps1 -Versions 1.20.2,1.21.3` or `-All`. Mappings and Fabric API are looked up automatically.
`src/compat/*` holds the few places where Minecraft's own code differs between versions.
1.21.5+ and 26.x need new compat folders (Minecraft changed GUI, input and key-binding code there).
