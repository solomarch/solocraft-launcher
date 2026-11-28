package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.option.KeyBinding;
import org.lwjgl.glfw.GLFW;

/** State and helpers for Zoom, Free Look and the Crosshair Editor. */
public final class Features {
    private Features() {}

    public static KeyBinding zoomKey, freeKey, perspectiveKey, waypointKey;

    public static final String[] CROSS_STYLES = {"Cross", "Dot", "Plus + dot", "Ring", "Corners"};
    public static final String[] CROSS_COLORS = {"White", "Red", "Green", "Cyan", "Yellow", "Pink"};
    private static final int[] COLORS = {0xFFFFFFFF, 0xFFFF4D4D, 0xFF4DFF6E, 0xFF4DE1FF, 0xFFFFE14D, 0xFFFF7AD9};

    public static boolean held(KeyBinding k, int fallbackCode) {
        // Poll GLFW as well as the keybinding state: some desktop/window-manager key grabs
        // (notably Alt on Linux) prevent Minecraft's keybinding state from updating reliably.
        return (k != null && k.isPressed()) || Input.keyDown(fallbackCode);
    }

    public static boolean zoomHeld() {
        return held(zoomKey, GLFW.GLFW_KEY_C);
    }

    public static boolean freeHeld() {
        return held(freeKey, GLFW.GLFW_KEY_LEFT_ALT);
    }

    // ---------------- Free Look ----------------
    public static volatile boolean freeActive;
    public static float freeYaw, freePitch;
    private static net.minecraft.client.option.Perspective savedPerspective;

    /** Starts Free Look: camera goes to third person behind the player and then orbits freely. */
    public static void freeBegin(MinecraftClient mc) {
        freeYaw = mc.player.getYaw();
        freePitch = mc.player.getPitch();
        try {
            savedPerspective = mc.options.getPerspective();
            if (savedPerspective.isFirstPerson() || savedPerspective.isFrontView()) {
                mc.options.setPerspective(net.minecraft.client.option.Perspective.THIRD_PERSON_BACK);
            } else {
                savedPerspective = null; // already third person back: leave it alone afterwards
            }
        } catch (Throwable t) {
            savedPerspective = null;
        }
        freeActive = true;
    }

    /** Ends Free Look and puts the view back the way it was. */
    public static void freeEnd(MinecraftClient mc) {
        freeActive = false;
        try {
            if (savedPerspective != null && mc != null) mc.options.setPerspective(savedPerspective);
        } catch (Throwable ignored) {
        }
        savedPerspective = null;
    }

    public static boolean freeLookOn() {
        return freeActive;
    }

    /** Called by the mouse-movement mixin: turns only the camera. */
    public static void freeLookMouse(double dx, double dy) {
        freeYaw += (float) dx * 0.15f;
        freePitch = Math.max(-90f, Math.min(90f, freePitch + (float) dy * 0.15f));
    }

    // ---------------- Crosshair ----------------
    public static boolean crosshairOn() {
        for (Module m : Modules.ALL) if (m.id.equals("crosshair")) return m.isEnabled() && !Hud.isBroken(m);
        return false;
    }

    public static void drawCrosshair(DrawContext c) {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc.currentScreen != null || mc.options.hudHidden) return;
        if (!mc.options.getPerspective().isFirstPerson()) return;
        Config cfg = Config.get();
        int cx = c.getScaledWindowWidth() / 2, cy = c.getScaledWindowHeight() / 2;
        int col = COLORS[cfg.crossColor], out = 0xAA000000, s = cfg.crossSize, gap = 2;
        switch (cfg.crossStyle) {
            case 1 -> dot(c, cx, cy, 1, col, out);
            case 2 -> {
                arms(c, cx, cy, gap + 1, s, col, out);
                dot(c, cx, cy, 0, col, out);
            }
            case 3 -> {
                int r = Math.max(3, s - 1);
                Gfx.border(c, cx - r - 1, cy - r - 1, 2 * r + 3, 2 * r + 3, out);
                Gfx.border(c, cx - r, cy - r, 2 * r + 1, 2 * r + 1, col);
            }
            case 4 -> {
                for (int sx = -1; sx <= 1; sx += 2) {
                    for (int sy = -1; sy <= 1; sy += 2) {
                        int x = cx + sx * s, y = cy + sy * s;
                        int hx1 = Math.min(x, x - sx * 3), hx2 = Math.max(x, x - sx * 3) + 1;
                        int vy1 = Math.min(y, y - sy * 3), vy2 = Math.max(y, y - sy * 3) + 1;
                        c.fill(hx1, y, hx2, y + 1, col);
                        c.fill(x, vy1, x + 1, vy2, col);
                    }
                }
            }
            default -> arms(c, cx, cy, gap, s, col, out);
        }
    }

    private static void arms(DrawContext c, int cx, int cy, int gap, int len, int col, int out) {
        // outline first so the colour stays readable on any background
        c.fill(cx - gap - len - 1, cy - 1, cx - gap + 1, cy + 2, out);
        c.fill(cx + gap, cy - 1, cx + gap + len + 1, cy + 2, out);
        c.fill(cx - 1, cy - gap - len - 1, cx + 2, cy - gap + 1, out);
        c.fill(cx - 1, cy + gap, cx + 2, cy + gap + len + 1, out);
        c.fill(cx - gap - len, cy, cx - gap, cy + 1, col);
        c.fill(cx + gap, cy, cx + gap + len, cy + 1, col);
        c.fill(cx, cy - gap - len, cx + 1, cy - gap, col);
        c.fill(cx, cy + gap, cx + 1, cy + gap + len, col);
    }

    private static void dot(DrawContext c, int cx, int cy, int r, int col, int out) {
        c.fill(cx - r - 1, cy - r - 1, cx + r + 2, cy + r + 2, out);
        c.fill(cx - r, cy - r, cx + r + 1, cy + r + 1, col);
    }
}
