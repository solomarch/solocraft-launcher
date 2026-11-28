package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

/** Drag HUD elements where you want them. Mouse and keys are read through Input (works on every 1.21.x). */
public class HudEditorScreen extends Screen {
    private Module dragging;
    private int offX, offY;
    private final Screen parent;
    private final Input.Edge mouse;
    private final Input.Edge shift;
    private final Input.Edge reset;
    private boolean failed;

    public HudEditorScreen(Screen parent) {
        super(Text.literal("Edit HUD Layout"));
        this.parent = parent;
        Input.beginScreen();
        mouse = new Input.Edge(Input.mouseDown(0));
        shift = new Input.Edge(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT));
        reset = new Input.Edge(Input.keyDown(GLFW.GLFW_KEY_R));
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public void render(DrawContext c, int mouseX, int mouseY, float delta) {
        try {
            draw(c, mouseX, mouseY);
        } catch (Throwable t) {
            SoloClient.LOG.error("HUD editor error", t);
            Chat.error("menu error, closing it: " + Chat.brief(t));
            if (!failed) {
                failed = true;
                MinecraftClient.getInstance().execute(this::close);
            }
        }
    }

    private void draw(DrawContext c, int mx, int my) {
        Input.sticky(true);
        // input first, so this frame already shows the result
        if (shift.rising(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT))) {
            close();
            return;
        }
        if (reset.rising(Input.keyDown(GLFW.GLFW_KEY_R))) {
            Config.get().positions.clear();
            Config.save();
        }
        boolean down = Input.mouseDown(0);
        if (mouse.rising(down)) {
            for (int i = Modules.ALL.size() - 1; i >= 0; i--) {
                Module m = Modules.ALL.get(i);
                if (!m.hasHud || !m.isEnabled() || m.lastW == 0) continue;
                if (mx >= m.lastX && mx <= m.lastX + m.lastW && my >= m.lastY && my <= m.lastY + m.lastH) {
                    dragging = m;
                    offX = mx - m.lastX;
                    offY = my - m.lastY;
                    break;
                }
            }
        } else if (dragging != null && down) {
            dragging.moveTo(mx - offX, my - offY);
            dragging.lastX = dragging.x();
            dragging.lastY = dragging.y();
        }
        if (dragging != null && !down) {
            dragging = null;
            Config.save();
        }

        // drawn without super.render: it would draw (and on 1.21 blur) the background and this screen has no widgets
        c.fill(0, 0, width, height, 0x55000000);
        Hud.drawAll(c, true);
        for (Module m : Modules.ALL) {
            if (!m.hasHud || !m.isEnabled() || m.lastW == 0 || Hud.isBroken(m)) continue;
            int col = m == dragging ? Config.get().accentColor() : 0xAAFFFFFF;
            Gfx.border(c, m.lastX - 1, m.lastY - 1, m.lastW + 2, m.lastH + 2, col);
        }
        String t = "Drag elements to move them.  Right Shift or Esc to finish.  Press R to reset all positions.";
        Gfx.text(c, Hud.tr(), t, (width - Hud.tr().getWidth(t)) / 2, 8, 0xFFFFFFFF, true);
    }

    @Override
    public void removed() {
        Input.sticky(false);
        Config.save();
    }

    @Override
    public void close() {
        Config.save();
        MinecraftClient mc = MinecraftClient.getInstance();
        mc.setScreen(parent);
    }
}
