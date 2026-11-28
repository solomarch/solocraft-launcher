package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** Shared drawing helpers and the HUD pass. */
public final class Hud {
    private Hud() {}

    /** Modules that threw an error on this Minecraft version are switched off for the session. */
    private static final Set<Module> BROKEN = new HashSet<>();

    public static boolean isBroken(Module m) {
        return BROKEN.contains(m);
    }

    public static void markBroken(Module m, Throwable t) {
        if (BROKEN.add(m)) {
            SoloClient.LOG.error("Module '{}' does not work on this Minecraft version and was switched off: {}", m.name, t.toString());
            Chat.error("a feature does not fit this Minecraft version and was switched off: " + m.name);
        }
    }

    public static TextRenderer tr() {
        return MinecraftClient.getInstance().textRenderer;
    }

    /** Draws every enabled HUD module at its saved position. */
    public static void drawAll(DrawContext c, boolean editing) {
        MinecraftClient mc = MinecraftClient.getInstance();
        int sw = mc.getWindow().getScaledWidth();
        int sh = mc.getWindow().getScaledHeight();
        float s = Gfx.canScale() ? Config.get().scale : 1f;
        // Modules the player has not moved are stacked automatically so nothing overlaps.
        int flowX = 4, flowY = 4, colW = 0;
        for (Module m : Modules.ALL) {
            if (!m.hasHud || !m.isEnabled() || isBroken(m)) continue;
            boolean placed = m.hasSavedPos();
            int x = placed ? Math.max(0, Math.min(m.x(), sw - 10)) : flowX;
            int y = placed ? Math.max(0, Math.min(m.y(), sh - 10)) : flowY;
            int[] size;
            Gfx.pushScale(c, s);
            try {
                size = m.draw(c, Math.round(x / s), Math.round(y / s), editing);
            } catch (Throwable t) {
                markBroken(m, t);
                continue;
            } finally {
                Gfx.pop(c);
            }
            m.lastX = x;
            m.lastY = y;
            m.lastW = Math.round(size[0] * s);
            m.lastH = Math.round(size[1] * s);
            if (!placed && m.lastH > 0) {
                colW = Math.max(colW, m.lastW);
                flowY += m.lastH + 4;
                if (flowY > sh - 40) {
                    flowX += colW + 8;
                    flowY = 4;
                    colW = 0;
                }
            }
        }
    }

    /** A box of text lines. Returns {width, height}. */
    public static int[] textBox(DrawContext c, int x, int y, List<String> lines) {
        TextRenderer t = tr();
        int w = 0;
        for (String l : lines) w = Math.max(w, t.getWidth(l));
        int pad = 3;
        int lh = t.fontHeight + 1;
        int bw = w + pad * 2;
        int bh = lines.size() * lh + pad * 2 - 1;
        if (Config.get().background) c.fill(x, y, x + bw, y + bh, 0x90000000);
        for (int i = 0; i < lines.size(); i++) {
            Gfx.text(c, t, lines.get(i), x + pad, y + pad + i * lh, 0xFFFFFFFF, Config.get().shadow);
        }
        return new int[]{bw, bh};
    }

    public static int[] oneLine(DrawContext c, int x, int y, String text) {
        return textBox(c, x, y, List.of(text));
    }

    /** Rectangle with slightly cut corners. */
    public static void round(DrawContext c, int x, int y, int w, int h, int color) {
        c.fill(x + 1, y, x + w - 1, y + h, color);
        c.fill(x, y + 1, x + w, y + h - 1, color);
    }
}
