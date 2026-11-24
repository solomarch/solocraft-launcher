package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.List;

/** Shared drawing helpers and the HUD pass. */
public final class Hud {
    private Hud() {}

    public static TextRenderer tr() {
        return MinecraftClient.getInstance().textRenderer;
    }

    /** Draws every enabled HUD module at its saved position. */
    public static void drawAll(DrawContext c, boolean editing) {
        MinecraftClient mc = MinecraftClient.getInstance();
        int sw = mc.getWindow().getScaledWidth();
        int sh = mc.getWindow().getScaledHeight();
        float s = Config.get().scale;
        // Modules the player has not moved are stacked automatically so nothing overlaps.
        int flowX = 4, flowY = 4, colW = 0;
        for (Module m : Modules.ALL) {
            if (!m.hasHud || !m.isEnabled()) continue;
            boolean placed = m.hasSavedPos();
            int x = placed ? Math.max(0, Math.min(m.x(), sw - 10)) : flowX;
            int y = placed ? Math.max(0, Math.min(m.y(), sh - 10)) : flowY;
            c.getMatrices().push();
            c.getMatrices().scale(s, s, 1f);
            int[] size = m.draw(c, Math.round(x / s), Math.round(y / s), editing);
            c.getMatrices().pop();
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
            c.drawText(t, lines.get(i), x + pad, y + pad + i * lh, 0xFFFFFFFF, Config.get().shadow);
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
