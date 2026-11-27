package dev.solocraft.client;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import net.fabricmc.loader.api.FabricLoader;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/** Saved to .minecraft/config/solocraft-client.json */
public final class Config {
    public Map<String, Boolean> enabled = new HashMap<>();
    public Map<String, int[]> positions = new HashMap<>();
    public Set<String> favourites = new HashSet<>();
    public float scale = 1.25f;      // HUD size
    public float menuScale = 1.5f;   // size of the menu window
    public boolean shadow = true;
    public boolean background = true;
    public int accent = 0;
    public int zoomLevel = 4;        // 2..10
    public int crossStyle = 0;
    public int crossColor = 0;
    public int crossSize = 6;        // 2..14

    public static final int[] ACCENTS = {0xFF6C5CE7, 0xFF2E86DE, 0xFF10AC84, 0xFFEE5A24, 0xFFE84393};
    public static final String[] ACCENT_NAMES = {"Purple", "Blue", "Green", "Orange", "Pink"};

    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static Config instance = new Config();

    public static Config get() {
        return instance;
    }

    private static Path file() {
        return FabricLoader.getInstance().getConfigDir().resolve("solocraft-client.json");
    }

    public static void load() {
        try {
            Path f = file();
            if (Files.exists(f)) {
                Config c = GSON.fromJson(Files.readString(f), Config.class);
                if (c != null) {
                    if (c.enabled == null) c.enabled = new HashMap<>();
                    if (c.positions == null) c.positions = new HashMap<>();
                    if (c.favourites == null) c.favourites = new HashSet<>();
                    if (c.scale < 0.5f || c.scale > 2.0f) c.scale = 1.25f;
                    if (c.menuScale < 0.8f || c.menuScale > 2.5f) c.menuScale = 1.5f;
                    c.zoomLevel = Math.max(2, Math.min(10, c.zoomLevel == 0 ? 4 : c.zoomLevel));
                    c.crossSize = Math.max(2, Math.min(14, c.crossSize == 0 ? 6 : c.crossSize));
                    c.crossStyle = Math.max(0, Math.min(Features.CROSS_STYLES.length - 1, c.crossStyle));
                    c.crossColor = Math.max(0, Math.min(Features.CROSS_COLORS.length - 1, c.crossColor));
                    if (c.accent < 0 || c.accent >= ACCENTS.length) c.accent = 0;
                    instance = c;
                }
            }
        } catch (Exception e) {
            SoloClient.LOG.warn("Could not read config, using defaults: {}", e.toString());
        }
    }

    public static void save() {
        try {
            Files.writeString(file(), GSON.toJson(instance));
        } catch (Exception e) {
            SoloClient.LOG.warn("Could not save config: {}", e.toString());
        }
    }

    public int accentColor() {
        return ACCENTS[accent];
    }
}
