package dev.solocraft.client;

import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.MinecraftClient;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;

/** Brings FPS Companion Max's pack diagnostics and one-click profile into the SM Craft menu. */
public final class PerformanceIntegration {
    private static final String[][] MODS = {
        {"sodium", "Sodium"}, {"sodium-extra", "Sodium Extra"}, {"lithium", "Lithium"},
        {"ferritecore", "FerriteCore"}, {"entityculling", "EntityCulling"},
        {"moreculling", "More Culling"}, {"immediatelyfast", "ImmediatelyFast"}, {"dynamic_fps", "Dynamic FPS"}
    };

    private PerformanceIntegration() {}

    public static List<String> installedOptimizationMods() {
        List<String> found = new ArrayList<>();
        for (String[] mod : MODS) if (FabricLoader.getInstance().isModLoaded(mod[0])) found.add(mod[1]);
        return found;
    }

    public static int optimizationModCount() { return installedOptimizationMods().size(); }
    public static int optimizationModTotal() { return MODS.length; }

    /** Applies and saves the settings from FPS Companion Max without overwriting higher chunk distances. */
    public static String applyMaxFpsProfile(MinecraftClient mc) {
        List<String> applied = new ArrayList<>(), skipped = new ArrayList<>();
        numeric(mc, "Render distance", new String[]{"getViewDistance", "getRenderDistance"}, 12, true, applied, skipped);
        numeric(mc, "Simulation distance", new String[]{"getSimulationDistance"}, 12, true, applied, skipped);
        enumValue(mc, "Particles", new String[]{"getParticles"}, "MINIMAL", applied, skipped);
        enumValue(mc, "Clouds", new String[]{"getCloudRenderMode", "getClouds"}, "OFF", applied, skipped);
        numeric(mc, "Entity distance", new String[]{"getEntityDistanceScaling"}, 0.5, false, applied, skipped);
        booleanValue(mc, "Entity shadows", new String[]{"getEntityShadows"}, false, applied, skipped);
        booleanValue(mc, "Ambient occlusion", new String[]{"getAo", "getAmbientOcclusion"}, false, applied, skipped);
        numeric(mc, "Biome blend", new String[]{"getBiomeBlendRadius"}, 0, false, applied, skipped);
        booleanValue(mc, "VSync", new String[]{"getEnableVsync"}, false, applied, skipped);
        numeric(mc, "FPS limit", new String[]{"getMaxFps"}, 260, false, applied, skipped);
        try { mc.options.write(); } catch (Throwable ignored) { skipped.add("saving options"); }
        String result = "Applied " + applied.size() + " max FPS settings; render and simulation distances are at least 12 chunks.";
        if (!skipped.isEmpty()) result += " Not available here: " + String.join(", ", skipped) + ".";
        return result;
    }

    private static Object option(MinecraftClient mc, String[] getters) throws Exception {
        for (String getter : getters) {
            for (Method method : mc.options.getClass().getMethods()) {
                if (method.getName().equals(getter) && method.getParameterCount() == 0) return method.invoke(mc.options);
            }
        }
        throw new NoSuchMethodException(getters[0]);
    }

    private static void numeric(MinecraftClient mc, String label, String[] getters, double wanted,
                                boolean minimum, List<String> applied, List<String> skipped) {
        try {
            Object opt = option(mc, getters), value = Gfx.rawGet(opt);
            if (!(value instanceof Number number)) throw new IllegalStateException("not numeric");
            double next = minimum ? Math.max(wanted, number.doubleValue()) : wanted;
            Object boxed;
            if (value instanceof Integer) boxed = (int) next;
            else if (value instanceof Long) boxed = (long) next;
            else if (value instanceof Float) boxed = (float) next;
            else boxed = next;
            Gfx.rawSet(opt, boxed);
            applied.add(label);
        } catch (Throwable ignored) { skipped.add(label); }
    }

    private static void booleanValue(MinecraftClient mc, String label, String[] getters, boolean next,
                                     List<String> applied, List<String> skipped) {
        try { Gfx.rawSet(option(mc, getters), next); applied.add(label); }
        catch (Throwable ignored) { skipped.add(label); }
    }

    private static void enumValue(MinecraftClient mc, String label, String[] getters, String target,
                                  List<String> applied, List<String> skipped) {
        try {
            Object opt = option(mc, getters), value = Gfx.rawGet(opt);
            if (!(value instanceof Enum<?> current)) throw new IllegalStateException("not an enum option");
            Object match = null;
            for (Object candidate : current.getDeclaringClass().getEnumConstants())
                if (candidate.toString().equalsIgnoreCase(target)) { match = candidate; break; }
            if (match == null) throw new IllegalStateException("value unavailable");
            Gfx.rawSet(opt, match);
            applied.add(label);
        } catch (Throwable ignored) { skipped.add(label); }
    }
}
