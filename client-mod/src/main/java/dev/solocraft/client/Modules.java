package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.network.ServerInfo;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.TntEntity;
import net.minecraft.util.hit.EntityHitResult;
import net.minecraft.entity.EquipmentSlot;
import net.minecraft.entity.effect.StatusEffectInstance;
import net.minecraft.item.ItemStack;
import net.minecraft.util.math.Direction;

import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.List;

/** Every module of the client. */
public final class Modules {
    public static final List<Module> ALL = new ArrayList<>();

    private Modules() {}

    private static MinecraftClient mc() {
        return MinecraftClient.getInstance();
    }

    /** A module that is just a few lines of text. */
    private abstract static class TextModule extends Module {
        TextModule(String id, String name, String desc, int x, int y, boolean on) {
            this(id, name, desc, "HUD", x, y, on);
        }

        TextModule(String id, String name, String desc, String category, int x, int y, boolean on) {
            super(id, name, desc, category, on, true, x, y);
        }

        abstract List<String> lines(MinecraftClient mc);

        @Override
        public int[] draw(DrawContext c, int x, int y, boolean editing) {
            MinecraftClient mc = mc();
            if (mc.player == null) return new int[]{0, 0};
            List<String> l = lines(mc);
            if (l == null || l.isEmpty()) {
                if (!editing) return new int[]{0, 0};
                l = List.of(name);
            }
            return Hud.textBox(c, x, y, l);
        }
    }

    public static void init() {
        if (!ALL.isEmpty()) return;

        ALL.add(new TextModule("fps", "FPS", "Shows your frames per second.", 4, 4, true) {
            List<String> lines(MinecraftClient mc) { return List.of(mc.getCurrentFps() + " FPS"); }
        });

        ALL.add(new TextModule("coords", "Coordinates", "Shows your block position and the way you are facing.", 4, 26, false) {
            List<String> lines(MinecraftClient mc) {
                return List.of("XYZ: " + mc.player.getBlockX() + " / " + mc.player.getBlockY() + " / " + mc.player.getBlockZ());
            }
        });

        ALL.add(new TextModule("cps", "CPS", "Shows how many times per second you click.", 4, 48, true) {
            List<String> lines(MinecraftClient mc) { return List.of(Tracker.left() + " | " + Tracker.right() + " CPS"); }
        });

        ALL.add(new TextModule("combo", "Combo Counter", "Counts entity attack actions within a short window; it does not automate attacks.", 260, 224, false) {
            List<String> lines(MinecraftClient mc) { int n = Tracker.combo(); return n == 0 ? List.of() : List.of("Combo: " + n); }
        });

        ALL.add(new Module("keys", "Keystrokes", "Shows your movement keys, mouse buttons and space bar.", "HUD", true, true, 4, 80) {
            @Override
            public int[] draw(DrawContext c, int x, int y, boolean editing) {
                MinecraftClient mc = mc();
                if (mc.player == null) return new int[]{0, 0};
                int s = 20, g = 2;
                key(c, x + s + g, y, s, s, "W", mc.options.forwardKey);
                key(c, x, y + s + g, s, s, "A", mc.options.leftKey);
                key(c, x + s + g, y + s + g, s, s, "S", mc.options.backKey);
                key(c, x + 2 * (s + g), y + s + g, s, s, "D", mc.options.rightKey);
                int hw = (3 * s + 2 * g - g) / 2;
                box(c, x, y + 2 * (s + g), hw, s, "LMB " + Tracker.left(), Tracker.leftDown());
                box(c, x + hw + g, y + 2 * (s + g), hw, s, "RMB " + Tracker.right(), Tracker.rightDown());
                box(c, x, y + 3 * (s + g), 3 * s + 2 * g, 10, "", mc.options.jumpKey.isPressed());
                return new int[]{3 * s + 2 * g, 3 * (s + g) + 10};
            }

            private void key(DrawContext c, int x, int y, int w, int h, String label, KeyBinding k) {
                box(c, x, y, w, h, label, k.isPressed());
            }

            private void box(DrawContext c, int x, int y, int w, int h, String label, boolean down) {
                int col = down ? (Config.get().accentColor() & 0x00FFFFFF) | 0xC0000000 : 0x90000000;
                Hud.round(c, x, y, w, h, col);
                if (!label.isEmpty()) {
                    int tw = Hud.tr().getWidth(label);
                    Gfx.text(c, Hud.tr(), label, x + (w - tw) / 2, y + (h - Hud.tr().fontHeight) / 2 + 1, 0xFFFFFFFF, Config.get().shadow);
                }
            }
        });

        ALL.add(new TextModule("ping", "Ping", "Shows your connection latency to the server.", 4, 150, false) {
            List<String> lines(MinecraftClient mc) {
                if (mc.isInSingleplayer()) return List.of("Local");
                int ms = 0;
                try {
                    if (mc.getNetworkHandler() != null && mc.player != null) {
                        PlayerListEntry e = mc.getNetworkHandler().getPlayerListEntry(mc.player.getUuid());
                        if (e != null) ms = e.getLatency();
                    }
                } catch (Exception ignored) {
                }
                if (ms <= 0) {
                    // the server has not reported our latency (yet): use the value from the server list
                    ServerInfo si = mc.getCurrentServerEntry();
                    if (si != null && si.ping > 0) ms = (int) si.ping;
                }
                return List.of(ms > 0 ? ms + " ms" : "... ms");
            }
        });

        ALL.add(new Module("armor", "Armor Status", "Shows your armour and held items with their durability.", "HUD", true, true, 4, 172) {
            @Override
            public int[] draw(DrawContext c, int x, int y, boolean editing) {
                MinecraftClient mc = mc();
                if (mc.player == null) return new int[]{0, 0};
                EquipmentSlot[] slots = {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET, EquipmentSlot.MAINHAND};
                int row = 0, maxW = 0;
                for (EquipmentSlot sl : slots) {
                    ItemStack st = mc.player.getEquippedStack(sl);
                    if (st.isEmpty()) continue;
                    int ry = y + row * 17;
                    c.drawItem(st, x, ry);
                    String t = "";
                    if (st.isDamageable()) t = String.valueOf(st.getMaxDamage() - st.getDamage());
                    else if (st.getCount() > 1) t = "x" + st.getCount();
                    if (!t.isEmpty()) {
                        Gfx.text(c, Hud.tr(), t, x + 19, ry + 4, 0xFFFFFFFF, Config.get().shadow);
                        maxW = Math.max(maxW, Hud.tr().getWidth(t) + 19);
                    }
                    maxW = Math.max(maxW, 18);
                    row++;
                }
                if (row == 0) {
                    if (!editing) return new int[]{0, 0};
                    return Hud.oneLine(c, x, y, "Armor");
                }
                return new int[]{maxW, row * 17};
            }
        });

        ALL.add(new TextModule("effects", "Potion Effects", "Shows your active potion effects and how long they last.", 4, 260, true) {
            List<String> lines(MinecraftClient mc) {
                List<String> out = new ArrayList<>();
                for (StatusEffectInstance e : mc.player.getStatusEffects()) {
                    String n = Compat.effectName(e);
                    int amp = e.getAmplifier() + 1;
                    String lvl = amp > 1 ? " " + roman(amp) : "";
                    String time = e.getDuration() < 0 ? "inf" : String.format("%d:%02d", e.getDuration() / 1200, (e.getDuration() / 20) % 60);
                    out.add(n + lvl + "  " + time);
                }
                return out;
            }
        });

        ALL.add(new TextModule("memory", "Memory", "Shows how much memory the game is using.", 4, 120, true) {
            List<String> lines(MinecraftClient mc) {
                Runtime r = Runtime.getRuntime();
                long used = (r.totalMemory() - r.freeMemory()) >> 20;
                long max = r.maxMemory() >> 20;
                return List.of("Mem: " + (used * 100 / Math.max(1, max)) + "% " + used + " MB");
            }
        });

        ALL.add(new TextModule("clock", "Clock", "Shows the time on your computer.", 4, 198, false) {
            List<String> lines(MinecraftClient mc) { return List.of(LocalTime.now().format(DateTimeFormatter.ofPattern("HH:mm"))); }
        });

        ALL.add(new TextModule("direction", "Direction", "Shows which way you are facing.", 4, 220, false) {
            List<String> lines(MinecraftClient mc) {
                Direction d = mc.player.getHorizontalFacing();
                String n = d.asString();
                n = Character.toUpperCase(n.charAt(0)) + n.substring(1);
                String axis = switch (d) {
                    case NORTH -> "-Z";
                    case SOUTH -> "+Z";
                    case EAST -> "+X";
                    case WEST -> "-X";
                    default -> "";
                };
                return List.of(n + " (" + axis + ")");
            }
        });

        ALL.add(new TextModule("day", "Day Counter", "Shows how many in-game days the world has been running.", 4, 240, false) {
            List<String> lines(MinecraftClient mc) {
                if (mc.world == null) return null;
                return List.of("Day " + (mc.world.getTimeOfDay() / 24000L + 1));
            }
        });

        ALL.add(new TextModule("server", "Server Address", "Shows the address of the server you are playing on.", 4, 282, false) {
            List<String> lines(MinecraftClient mc) {
                ServerInfo si = mc.getCurrentServerEntry();
                return List.of(si != null ? si.address : "Singleplayer");
            }
        });

        ALL.add(new TextModule("speed", "Speed", "Shows your current horizontal movement speed.", 260, 4, false) {
            private double x, z, speed;
            private boolean sampled;
            @Override public void tick(MinecraftClient mc) {
                if (mc.player == null) { sampled = false; speed = 0; return; }
                double nx = mc.player.getX(), nz = mc.player.getZ();
                if (sampled) speed = Math.min(100, Math.sqrt((nx - x) * (nx - x) + (nz - z) * (nz - z)) * 20.0);
                x = nx; z = nz; sampled = true;
            }
            List<String> lines(MinecraftClient mc) { return List.of(String.format(java.util.Locale.ROOT, "Speed: %.2f blocks/s", speed)); }
        });

        ALL.add(new TextModule("saturation", "Saturation", "Shows your current food saturation.", 260, 26, false) {
            List<String> lines(MinecraftClient mc) { return List.of(String.format(java.util.Locale.ROOT, "Saturation: %.1f", mc.player.getHungerManager().getSaturationLevel())); }
        });

        ALL.add(new TextModule("itemcount", "Item Counter", "Shows the selected item's name and stack count.", 260, 48, false) {
            List<String> lines(MinecraftClient mc) {
                ItemStack st = mc.player.getMainHandStack();
                return st.isEmpty() ? List.of("Empty hand") : List.of(st.getName().getString() + " x" + st.getCount());
            }
        });

        ALL.add(new TextModule("biome", "Biome", "Shows the biome you are standing in.", 260, 70, false) {
            List<String> lines(MinecraftClient mc) {
                return List.of("Biome: " + mc.world.getBiome(mc.player.getBlockPos()).getKey()
                    .map(k -> k.getValue().toString()).orElse("unknown"));
            }
        });

        ALL.add(new TextModule("light", "Light Level", "Shows the block light level at your position.", 260, 92, false) {
            List<String> lines(MinecraftClient mc) { return List.of("Light: " + mc.world.getLightLevel(mc.player.getBlockPos())); }
        });

        ALL.add(new TextModule("reachdisplay", "Reach Display", "Displays distance to your current crosshair target; it does not change reach.", 260, 114, false) {
            List<String> lines(MinecraftClient mc) {
                if (mc.crosshairTarget == null || mc.crosshairTarget.getType().name().equals("MISS")) return List.of();
                double d = mc.player.getEyePos().distanceTo(mc.crosshairTarget.getPos());
                return List.of(String.format(java.util.Locale.ROOT, "Target distance: %.2f blocks", d));
            }
        });

        ALL.add(new TextModule("durabilityalerts", "Durability Alerts", "Warns when equipped or hotbar gear is close to breaking.", 260, 136, false) {
            List<String> lines(MinecraftClient mc) {
                List<String> out = new ArrayList<>();
                var inv = mc.player.getInventory();
                for (int i = 0; i < inv.size() && out.size() < 4; i++) {
                    ItemStack st = inv.getStack(i);
                    if (st.isDamageable() && st.getMaxDamage() - st.getDamage() <= Math.max(1, st.getMaxDamage() / 10))
                        out.add("Low durability: " + st.getName().getString());
                }
                return out;
            }
        });

        ALL.add(new TextModule("targethealth", "Target Health", "Shows health points and hearts for the living mob or player under your crosshair.", 260, 158, true) {
            List<String> lines(MinecraftClient mc) {
                if (!(mc.crosshairTarget instanceof EntityHitResult hit) || !(hit.getEntity() instanceof LivingEntity target)) return List.of();
                float health = Math.max(0, target.getHealth());
                float maxHealth = Math.max(1, target.getMaxHealth());
                int hearts = Math.round(health / 2f), maxHearts = Math.max(1, Math.round(maxHealth / 2f));
                return List.of(String.format(java.util.Locale.ROOT, "%s: %.1f / %.1f HP (%d / %d ♥)",
                    target.getName().getString(), health, maxHealth, hearts, maxHearts));
            }
        });

        ALL.add(new TextModule("tnttimer", "TNT Timer", "Shows the fuse time of the nearest primed TNT within 16 blocks.", 260, 246, false) {
            List<String> lines(MinecraftClient mc) {
                TntEntity tnt = mc.world.getEntitiesByClass(TntEntity.class, mc.player.getBoundingBox().expand(16), e -> true)
                    .stream().min(java.util.Comparator.comparingDouble(e -> mc.player.squaredDistanceTo(e))).orElse(null);
                return tnt == null ? List.of() : List.of(String.format(java.util.Locale.ROOT, "TNT: %.1f s", Math.max(0, tnt.getFuse()) / 20.0));
            }
        });

        ALL.add(new TextModule("packdisplay", "Pack Display", "Lists enabled resource packs.", 260, 268, false) {
            List<String> lines(MinecraftClient mc) {
                try {
                    Object manager = mc.getResourcePackManager();
                    for (var method : manager.getClass().getMethods()) {
                        if (method.getName().equals("getEnabledProfiles") && method.getParameterCount() == 0) {
                            Object value = method.invoke(manager);
                            if (value instanceof java.util.Collection<?> profiles) {
                                List<String> out = new ArrayList<>();
                                for (Object profile : profiles) {
                                    Object id = profile.getClass().getMethod("getId").invoke(profile);
                                    out.add(id.toString());
                                }
                                if (!out.isEmpty()) return out;
                            }
                        }
                    }
                } catch (Throwable ignored) { }
                return List.of("No enabled resource packs");
            }
        });

        ALL.add(new Module("waypoint", "Waypoint", "Press X to save your current position and show the distance to it.", "Utility", false, true, 260, 202) {
            @Override public void tick(MinecraftClient mc) {
                if (isEnabled() && mc.player != null && mc.currentScreen == null && Features.waypointKey != null) {
                    while (Features.waypointKey.wasPressed()) {
                        Config.get().waypoint = new int[]{mc.player.getBlockX(), mc.player.getBlockY(), mc.player.getBlockZ()};
                        Config.save();
                    }
                }
            }
            @Override public int[] draw(DrawContext c, int x, int y, boolean editing) {
                MinecraftClient mc = mc();
                int[] p = Config.get().waypoint;
                if (mc.player == null || p == null) return editing ? Hud.oneLine(c, x, y, "Waypoint (press X)") : new int[]{0, 0};
                double d = mc.player.squaredDistanceTo(p[0] + .5, p[1] + .5, p[2] + .5);
                return Hud.textBox(c, x, y, List.of("Waypoint: " + p[0] + " " + p[1] + " " + p[2], String.format(java.util.Locale.ROOT, "%.1f blocks", Math.sqrt(d))));
            }
        });

        ALL.add(new TextModule("lagspike", "Lag Spike Detector", "Shows the most recent client tick delay over 100 ms.", "Performance", 260, 180, false) {
            private long last, spike, seen;
            @Override public void tick(MinecraftClient mc) {
                long now = System.nanoTime();
                if (last != 0) {
                    long ms = (now - last) / 1_000_000;
                    if (ms > 100) { spike = ms; seen = now; }
                }
                last = now;
                if (now - seen > 5_000_000_000L) spike = 0;
            }
            List<String> lines(MinecraftClient mc) { return spike == 0 ? List.of() : List.of("Tick spike: " + spike + " ms"); }
        });

        ALL.add(new TextModule("framediagnostics", "Frame Diagnostics", "Shows a rolling 10-second average FPS, frame time, and 1% low.", "Performance", 260, 290, false) {
            List<String> lines(MinecraftClient mc) {
                if (FpsMetrics.windowSeconds() < 1) return List.of("Measuring frame times…");
                String low = FpsMetrics.hasFullWindow() ? FpsMetrics.onePercentLow() + " FPS" : "collecting";
                return List.of("Avg: " + FpsMetrics.averageFps() + " FPS | 1% low: " + low,
                    String.format(java.util.Locale.ROOT, "Frame time: %.2f ms", FpsMetrics.averageMs()));
            }
        });

        ALL.add(new TextModule("optimizationmods", "Optimization Mods", "Shows performance mods detected in this instance.", "Performance", 260, 314, false) {
            List<String> lines(MinecraftClient mc) {
                List<String> found = PerformanceIntegration.installedOptimizationMods();
                return found.isEmpty() ? List.of("Optimizations: 0 / " + PerformanceIntegration.optimizationModTotal())
                    : List.of("Optimizations: " + found.size() + " / " + PerformanceIntegration.optimizationModTotal(), String.join(", ", found));
            }
        });

        ALL.add(new ActionModule("maxfpsprofile", "Max FPS Profile",
            "Applies FPS Companion Max settings, keeps render and simulation distance at 12+ chunks, and detects optimization mods.",
            "Performance", PerformanceIntegration::applyMaxFpsProfile));

        // ---- mechanics ----
        ALL.add(new Module("sprint", "Toggle Sprint", "Always sprints while you walk forward.", "Utility", false, true, 4, 304) {
            @Override
            public void tick(MinecraftClient mc) {
                if (!isEnabled() || mc.player == null) return;
                if (mc.options.forwardKey.isPressed() && !mc.player.isSneaking() && !mc.player.horizontalCollision
                        && !mc.player.isUsingItem() && mc.player.getHungerManager().getFoodLevel() > 6) {
                    mc.player.setSprinting(true);
                }
            }

            @Override
            public int[] draw(DrawContext c, int x, int y, boolean editing) {
                MinecraftClient mc = mc();
                if (mc.player == null) return new int[]{0, 0};
                if (!mc.player.isSprinting() && !editing) return new int[]{0, 0};
                return Hud.oneLine(c, x, y, "[Sprinting (Toggled)]");
            }
        });

        ALL.add(new Module("invhud", "Inventory HUD", "Shows your inventory on screen, with item counts and durability.", "HUD", false, true, 4, 200) {
            @Override
            public int[] draw(DrawContext c, int x, int y, boolean editing) {
                MinecraftClient mc = mc();
                if (mc.player == null) return new int[]{0, 0};
                int w = 9 * 18 + 6, h = 3 * 18 + 6;
                if (Config.get().background) Hud.round(c, x, y, w, h, 0x90000000);
                var inv = mc.player.getInventory();
                for (int i = 0; i < 27; i++) {
                    ItemStack st = inv.getStack(9 + i);
                    if (st.isEmpty()) continue;
                    int sx = x + 3 + (i % 9) * 18, sy = y + 3 + (i / 9) * 18;
                    c.drawItem(st, sx, sy);
                    Gfx.stackOverlay(c, Hud.tr(), st, sx, sy); // count + durability only, never the item name
                }
                return new int[]{w, h};
            }
        });

        ALL.add(new Module("zoom", "Zoom", "Hold the Zoom key (C) to zoom in. Level is set in Settings.", "Visual", true, false, 0, 0) {
            private Object savedFov, savedSens;
            private boolean zooming;

            private void restore(MinecraftClient mc) {
                try {
                    if (savedFov != null) Gfx.rawSet(mc.options.getFov(), savedFov);
                    if (savedSens != null) Gfx.rawSet(mc.options.getMouseSensitivity(), savedSens);
                } catch (Exception ignored) {
                }
                savedFov = savedSens = null;
                zooming = false;
            }

            @Override
            public void stop(MinecraftClient mc) {
                restore(mc);
            }

            @Override
            public void tick(MinecraftClient mc) {
                try {
                    boolean want = isEnabled() && mc.currentScreen == null && Features.zoomHeld();
                    if (want) {
                        if (!zooming) {
                            savedFov = Gfx.rawGet(mc.options.getFov());
                            savedSens = Gfx.rawGet(mc.options.getMouseSensitivity());
                            zooming = true;
                        }
                        int lvl = Config.get().zoomLevel;
                        Gfx.rawSet(mc.options.getFov(), Integer.valueOf(Math.max(5, ((Integer) savedFov) / lvl)));
                        Gfx.rawSet(mc.options.getMouseSensitivity(), Double.valueOf(((Double) savedSens) / lvl));
                    } else if (zooming) {
                        restore(mc);
                    }
                } catch (Exception e) {
                    throw new RuntimeException(e);
                }
            }
        });

        ALL.add(new Module("freelook", "Free Look", "Hold the Free Look key (Left Alt) to look around without turning your player.", "Visual", true, false, 0, 0) {
            @Override
            public void stop(MinecraftClient mc) {
                Features.freeEnd(mc);
            }

            @Override
            public void tick(MinecraftClient mc) {
                boolean want = isEnabled() && mc.currentScreen == null && Features.freeHeld();
                if (want && !Features.freeActive) {
                    Features.freeBegin(mc);
                } else if (!want && Features.freeActive) {
                    Features.freeEnd(mc);
                }
            }
        });

        ALL.add(new Module("togglesneak", "Toggle Sneak", "Press the sneak key once to sneak, again to stop.", "Utility", false, false, 0, 0) {
            private Boolean saved;

            @Override
            public void stop(MinecraftClient mc) {
                try {
                    if (saved != null) Gfx.rawSet(mc.options.getSneakToggled(), saved);
                } catch (Exception ignored) {
                }
                saved = null;
            }

            @Override
            public void tick(MinecraftClient mc) {
                try {
                    Object opt = mc.options.getSneakToggled();
                    if (isEnabled()) {
                        if (saved == null) saved = (Boolean) Gfx.rawGet(opt);
                        Gfx.rawSet(opt, Boolean.TRUE);
                    } else if (saved != null) {
                        Gfx.rawSet(opt, saved);
                        saved = null;
                    }
                } catch (Exception e) {
                    throw new RuntimeException(e);
                }
            }
        });

        ALL.add(new Module("perspective", "Quick Perspective", "Press V to switch between first person and rear third person.", "Utility", true, false, 0, 0) {
            @Override public void tick(MinecraftClient mc) {
                if (!isEnabled() || mc.currentScreen != null || Features.perspectiveKey == null) return;
                while (Features.perspectiveKey.wasPressed()) {
                    var current = mc.options.getPerspective();
                    mc.options.setPerspective(current.isFirstPerson()
                        ? net.minecraft.client.option.Perspective.THIRD_PERSON_BACK
                        : net.minecraft.client.option.Perspective.FIRST_PERSON);
                }
            }
        });

        ALL.add(new Module("crosshair", "Crosshair Editor", "Replaces the crosshair with your own: shape, size and colour in Settings.", "Visual", false, false, 0, 0) {
        });

        ALL.add(new Module("fullbright", "Fullbright", "Makes everything fully lit, even in caves.", "Visual", false, false, 0, 0) {
            private Object saved;
            private java.lang.reflect.Field fld;

            private java.lang.reflect.Field field(Object opt) {
                if (fld == null) {
                    for (java.lang.reflect.Field f : opt.getClass().getDeclaredFields()) {
                        if (!java.lang.reflect.Modifier.isStatic(f.getModifiers()) && !java.lang.reflect.Modifier.isFinal(f.getModifiers())
                                && f.getType() == Object.class) {
                            f.setAccessible(true);
                            fld = f;
                            break;
                        }
                    }
                }
                return fld;
            }

            @Override
            public void stop(MinecraftClient mc) {
                try {
                    Object opt = mc.options.getGamma();
                    java.lang.reflect.Field f = field(opt);
                    if (f != null && saved != null) {
                        f.set(opt, saved);
                        saved = null;
                    }
                } catch (Exception ignored) {
                }
            }

            @Override
            public void tick(MinecraftClient mc) {
                try {
                    Object opt = mc.options.getGamma();
                    java.lang.reflect.Field f = field(opt);
                    if (f == null) return;
                    if (isEnabled()) {
                        if (saved == null) saved = f.get(opt);
                        f.set(opt, 16.0);
                    } else if (saved != null) {
                        f.set(opt, saved);
                        saved = null;
                    }
                } catch (Exception ignored) {
                    // keep the game running
                }
            }
        });

        ALL.add(new Module("hurtcam", "No Hurt Camera", "Disables the camera tilt when you take damage.", "Visual", false, false, 0, 0) {
            private Object saved;
            private Object option(MinecraftClient mc) throws Exception {
                for (var method : mc.options.getClass().getMethods()) {
                    if (method.getParameterCount() == 0 && method.getName().toLowerCase(java.util.Locale.ROOT).contains("damagetilt"))
                        return method.invoke(mc.options);
                }
                throw new NoSuchMethodException("damage tilt option");
            }
            @Override public void stop(MinecraftClient mc) { restore(mc); }
            private void restore(MinecraftClient mc) {
                if (saved == null) return;
                try { Gfx.rawSet(option(mc), saved); } catch (Exception ignored) {}
                saved = null;
            }
            @Override public void tick(MinecraftClient mc) {
                try {
                    Object opt = option(mc);
                    if (isEnabled()) {
                        if (saved == null) saved = Gfx.rawGet(opt);
                        Gfx.rawSet(opt, saved instanceof Double ? 0.0d : 0.0f);
                    } else if (saved != null) restore(mc);
                } catch (Exception ignored) { /* unavailable versions simply leave the option alone */ }
            }
        });

        ALL.add(new Module("unfocusedfps", "Unfocused FPS Limit", "Temporarily caps the game at 30 FPS while its window is unfocused.", "Performance", false, false, 0, 0) {
            private Object saved;
            private Object option(MinecraftClient mc) throws Exception {
                for (var method : mc.options.getClass().getMethods())
                    if (method.getName().equals("getMaxFps") && method.getParameterCount() == 0) return method.invoke(mc.options);
                throw new NoSuchMethodException("max FPS option");
            }
            private void restore(MinecraftClient mc) {
                if (saved == null) return;
                try { Gfx.rawSet(option(mc), saved); } catch (Exception ignored) {}
                saved = null;
            }
            @Override public void stop(MinecraftClient mc) { restore(mc); }
            @Override public void tick(MinecraftClient mc) {
                try {
                    Object opt = option(mc);
                    if (!isEnabled()) { restore(mc); return; }
                    if (saved == null) saved = Gfx.rawGet(opt);
                    if (!mc.isWindowFocused() && saved instanceof Integer fps) Gfx.rawSet(opt, Math.min(fps, 30));
                    else Gfx.rawSet(opt, saved);
                } catch (Exception ignored) { }
            }
        });

        ALL.add(new Module("particlelimiter", "Particle Limiter", "Temporarily uses minimal particles to reduce particle load.", "Performance", false, false, 0, 0) {
            private Object saved;
            private Object option(MinecraftClient mc) throws Exception {
                for (var method : mc.options.getClass().getMethods())
                    if (method.getName().equals("getParticles") && method.getParameterCount() == 0) return method.invoke(mc.options);
                throw new NoSuchMethodException("particle option");
            }
            private void restore(MinecraftClient mc) {
                if (saved == null) return;
                try { Gfx.rawSet(option(mc), saved); } catch (Exception ignored) {}
                saved = null;
            }
            @Override public void stop(MinecraftClient mc) { restore(mc); }
            @Override public void tick(MinecraftClient mc) {
                try {
                    Object opt = option(mc);
                    if (!isEnabled()) { restore(mc); return; }
                    if (saved == null) saved = Gfx.rawGet(opt);
                    if (saved instanceof Enum<?> e) {
                        for (Object value : e.getDeclaringClass().getEnumConstants())
                            if (value.toString().equalsIgnoreCase("MINIMAL")) { Gfx.rawSet(opt, value); break; }
                    }
                } catch (Exception ignored) { }
            }
        });
    }

    static String roman(int n) {
        String[] r = {"", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"};
        return n >= 1 && n <= 10 ? r[n] : String.valueOf(n);
    }
}
