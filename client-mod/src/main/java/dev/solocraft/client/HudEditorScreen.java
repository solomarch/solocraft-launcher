package dev.solocraft.client;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

/** Drag HUD elements where you want them. */
public class HudEditorScreen extends Screen {
    private Module dragging;
    private final Screen parent;

    public HudEditorScreen(Screen parent) {
        super(Text.literal("Edit HUD Layout"));
        this.parent = parent;
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public void render(DrawContext c, int mouseX, int mouseY, float delta) {
        c.fill(0, 0, width, height, 0x55000000);
        Hud.drawAll(c, true);
        for (Module m : Modules.ALL) {
            if (!m.hasHud || !m.isEnabled() || m.lastW == 0) continue;
            int col = m == dragging ? Config.get().accentColor() : 0xAAFFFFFF;
            c.drawBorder(m.lastX - 1, m.lastY - 1, m.lastW + 2, m.lastH + 2, col);
        }
        String t = "Drag elements to move them.  Right Shift or Esc to finish.  Press R to reset all positions.";
        c.drawCenteredTextWithShadow(textRenderer, t, width / 2, 8, 0xFFFFFFFF);
        // super.render is skipped on purpose: it would draw (and on 1.21 blur) the background,
        // and this screen has no widgets.
    }

    @Override
    public boolean mouseClicked(double mx, double my, int button) {
        if (button == 0) {
            for (int i = Modules.ALL.size() - 1; i >= 0; i--) {
                Module m = Modules.ALL.get(i);
                if (!m.hasHud || !m.isEnabled() || m.lastW == 0) continue;
                if (mx >= m.lastX && mx <= m.lastX + m.lastW && my >= m.lastY && my <= m.lastY + m.lastH) {
                    dragging = m;
                    return true;
                }
            }
        }
        return super.mouseClicked(mx, my, button);
    }

    @Override
    public boolean mouseDragged(double mx, double my, int button, double dx, double dy) {
        if (dragging != null) {
            dragging.moveTo((int) Math.round(dragging.lastX + dx), (int) Math.round(dragging.lastY + dy));
            dragging.lastX = dragging.x();
            dragging.lastY = dragging.y();
            return true;
        }
        return super.mouseDragged(mx, my, button, dx, dy);
    }

    @Override
    public boolean mouseReleased(double mx, double my, int button) {
        if (dragging != null) {
            dragging = null;
            Config.save();
            return true;
        }
        return super.mouseReleased(mx, my, button);
    }

    @Override
    public boolean keyPressed(int key, int scan, int mods) {
        if (key == GLFW.GLFW_KEY_RIGHT_SHIFT) {
            close();
            return true;
        }
        if (key == GLFW.GLFW_KEY_R) {
            Config.get().positions.clear();
            Config.save();
            return true;
        }
        return super.keyPressed(key, scan, mods);
    }

    @Override
    public void close() {
        Config.save();
        if (client != null) client.setScreen(parent);
    }
}
