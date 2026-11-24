# SoloCraft Client (Fabric mod)

In-game menu and HUD for SoloCraft Launcher. Press **Right Shift** in game.

Supported Minecraft versions (Fabric): 1.20 up to 1.21.4. Not yet: 1.21.5 and newer, 26.x.

## Build (needs JDK 21 active; internet for the first build)

    .\build-all.ps1          builds every version into .\dist  (PowerShell)
    .\gradlew.bat build -Pmc=1.21.4      one version only

SoloCraft Launcher finds the jars in `client-mod\dist` by itself
(Mods window -> SoloCraft Client -> Install). If a version fails to build, the script
carries on with the others and lists what failed at the end.

## Other versions
`.\build-all.ps1 -Versions 1.20.2,1.21.3` or `-All`. Mappings and Fabric API are looked up automatically.
`src/compat/*` holds the few places where Minecraft's own code differs between versions.
1.21.5+ and 26.x need new compat folders (Minecraft changed GUI, input and key-binding code there).
