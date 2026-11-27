package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.network.ServerInfo;
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
            super(id, name, desc, "HUD", on, true, x, y);
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

        // ---- mechanics ----
        ALL.add(new Module("sprint", "Toggle Sprint", "Always sprints while you walk forward.", "Mechanic", false, true, 4, 304) {
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

        ALL.add(new Module("zoom", "Zoom", "Hold the Zoom key (C) to zoom in. Level is set in Settings.", "Mechanic", true, false, 0, 0) {
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

        ALL.add(new Module("freelook", "Free Look", "Hold the Free Look key (Left Alt) to look around without turning your player.", "Mechanic", true, false, 0, 0) {
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

        ALL.add(new Module("togglesneak", "Toggle Sneak", "Press the sneak key once to sneak, again to stop.", "Mechanic", false, false, 0, 0) {
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

        ALL.add(new Module("crosshair", "Crosshair Editor", "Replaces the crosshair with your own: shape, size and colour in Settings.", "Mechanic", false, false, 0, 0) {
        });

        ALL.add(new Module("fullbright", "Fullbright", "Makes everything fully lit, even in caves.", "Mechanic", false, false, 0, 0) {
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
    }

    static String roman(int n) {
        String[] r = {"", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"};
        return n >= 1 && n <= 10 ? r[n] : String.valueOf(n);
    }
}
