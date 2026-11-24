package dev.solocraft.client;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.TextFieldWidget;
import net.minecraft.text.OrderedText;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/** The Doki-style menu: sidebar, search, grid of module cards, settings. */
public class MenuScreen extends Screen {
    private static final int BG = 0xF0141418;
    private static final int PANEL = 0xFF1B1B21;
    private static final int CARD = 0xFF202027;
    private static final int CARD_HOVER = 0xFF26262E;
    private static final int TEXT = 0xFFE6E6EA;
    private static final int MUTED = 0xFF8B8B96;

    private enum Tab { MODS, SETTINGS }

    private Tab tab = Tab.MODS;
    private String category = "All";
    private TextFieldWidget search;
    private int px, py, pw, ph;
    private int scroll;
    private int contentHeight;

    // click targets rebuilt each frame
    private record Hit(int x, int y, int w, int h, Runnable action) {
        boolean in(double mx, double my) { return mx >= x && mx < x + w && my >= y && my < y + h; }
    }
    private final List<Hit> hits = new ArrayList<>();

    public MenuScreen() {
        super(Text.literal("SoloCraft Client"));
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    protected void init() {
        pw = Math.min(width - 16, 540);
        ph = Math.min(height - 16, 340);
        px = (width - pw) / 2;
        py = (height - ph) / 2;
        String old = search != null ? search.getText() : "";
        search = new TextFieldWidget(textRenderer, px + 126, py + 40, pw - 126 - 14, 16, Text.literal("Search"));
        search.setSuggestion("Search mods...");
        search.setText(old);
        search.setChangedListener(s -> {
            scroll = 0;
            search.setSuggestion(s.isEmpty() ? "Search mods..." : "");
        });
        search.setSuggestion(old.isEmpty() ? "Search mods..." : "");
        addDrawableChild(search);
    }

    private List<Module> visible() {
        String q = search == null ? "" : search.getText().trim().toLowerCase();
        List<Module> out = new ArrayList<>();
        for (Module m : Modules.ALL) {
            boolean catOk = switch (category) {
                case "All" -> true;
                case "Favourites" -> Config.get().favourites.contains(m.id);
                default -> m.category.equals(category);
            };
            if (!catOk) continue;
            if (!q.isEmpty() && !(m.name.toLowerCase().contains(q) || m.description.toLowerCase().contains(q))) continue;
            out.add(m);
        }
        return out;
    }

    private int count(String cat) {
        int n = 0;
        for (Module m : Modules.ALL) {
            if (cat.equals("All") || (cat.equals("Favourites") ? Config.get().favourites.contains(m.id) : m.category.equals(cat))) n++;
        }
        return n;
    }

    @Override
    public void render(DrawContext c, int mx, int my, float delta) {
        Compat.menuBackground(this, c); // only needed on 1.20.1
        super.render(c, mx, my, delta); // background + search box
        hits.clear();
        int accent = Config.get().accentColor();

        Hud.round(c, px, py, pw, ph, BG);
        // header
        c.drawText(textRenderer, "SoloCraft Client", px + 12, py + 11, TEXT, true);
        int tx = px + 140;
        tx = tabButton(c, tx, py + 6, "Mods", Tab.MODS, mx, my, accent);
        tx = tabButton(c, tx, py + 6, "Settings", Tab.SETTINGS, mx, my, accent);
        c.drawText(textRenderer, "Waypoints", tx + 4, py + 11, 0xFF55555E, false);
        c.drawText(textRenderer, "Cosmetics", tx + 70, py + 11, 0xFF55555E, false);
        String close = "x";
        int cx = px + pw - 18;
        c.drawText(textRenderer, close, cx, py + 11, MUTED, false);
        hits.add(new Hit(cx - 4, py + 6, 18, 18, this::close));
        c.fill(px + 1, py + 28, px + pw - 1, py + 29, 0xFF2A2A32);

        // sidebar
        int sy = py + 38;
        String[] cats = {"All", "Favourites", "HUD", "Mechanic"};
        String[] labels = {"All Mods", "Favourites", "HUD", "Mechanic"};
        for (int i = 0; i < cats.length; i++) {
            final String cat = cats[i];
            boolean on = tab == Tab.MODS && category.equals(cat);
            int bx = px + 8, bw = 104;
            boolean hov = mx >= bx && mx < bx + bw && my >= sy && my < sy + 20;
            if (on || hov) Hud.round(c, bx, sy, bw, 20, on ? PANEL : 0xFF18181D);
            c.drawText(textRenderer, labels[i], bx + 8, sy + 6, on ? TEXT : MUTED, false);
            String n = String.valueOf(count(cat));
            c.drawText(textRenderer, n, bx + bw - 8 - textRenderer.getWidth(n), sy + 6, MUTED, false);
            hits.add(new Hit(bx, sy, bw, 20, () -> { tab = Tab.MODS; category = cat; scroll = 0; }));
            sy += 24;
        }
        // edit layout button
        int ey = py + ph - 32;
        boolean eh = mx >= px + 8 && mx < px + 112 && my >= ey && my < ey + 22;
        Hud.round(c, px + 8, ey, 104, 22, eh ? CARD_HOVER : CARD);
        String el = "Edit HUD Layout";
        c.drawText(textRenderer, el, px + 8 + (104 - textRenderer.getWidth(el)) / 2, ey + 7, TEXT, false);
        hits.add(new Hit(px + 8, ey, 104, 22, () -> {
            if (client != null) client.setScreen(new HudEditorScreen(this));
        }));

        search.visible = tab == Tab.MODS;
        search.setFocused(search.isFocused() && tab == Tab.MODS);
        if (tab == Tab.MODS) drawMods(c, mx, my, accent);
        else drawSettings(c, mx, my, accent);
    }

    private int tabButton(DrawContext c, int x, int y, String label, Tab t, int mx, int my, int accent) {
        int w = textRenderer.getWidth(label) + 20;
        boolean on = tab == t;
        boolean hov = mx >= x && mx < x + w && my >= y && my < y + 18;
        if (on) Hud.round(c, x, y, w, 18, CARD);
        else if (hov) Hud.round(c, x, y, w, 18, 0xFF18181D);
        c.drawText(textRenderer, label, x + 10, y + 5, on ? TEXT : MUTED, false);
        hits.add(new Hit(x, y, w, 18, () -> tab = t));
        return x + w + 4;
    }

    private void drawMods(DrawContext c, int mx, int my, int accent) {
        int gx = px + 126, gy = py + 62, gw = pw - 126 - 14, gh = ph - 62 - 10;
        int cols = gw >= 360 ? 3 : 2;
        int gap = 8;
        int cw = (gw - gap * (cols - 1)) / cols;
        int chh = 54;
        List<Module> list = visible();
        int rows = (list.size() + cols - 1) / cols;
        contentHeight = rows * (chh + gap);
        scroll = Math.max(0, Math.min(scroll, Math.max(0, contentHeight - gh)));

        c.enableScissor(gx, gy, gx + gw, gy + gh);
        for (int i = 0; i < list.size(); i++) {
            Module m = list.get(i);
            int x = gx + (i % cols) * (cw + gap);
            int y = gy + (i / cols) * (chh + gap) - scroll;
            if (y + chh < gy || y > gy + gh) continue;
            boolean inView = my >= gy && my < gy + gh;
            boolean hov = inView && mx >= x && mx < x + cw && my >= y && my < y + chh;
            Hud.round(c, x, y, cw, chh, hov ? CARD_HOVER : CARD);
            c.drawText(textRenderer, m.name, x + 8, y + 7, TEXT, false);
            // description (wrapped, max 2 lines)
            List<OrderedText> lines = textRenderer.wrapLines(Text.literal(m.description), cw - 16);
            for (int l = 0; l < Math.min(2, lines.size()); l++) {
                c.drawText(textRenderer, lines.get(l), x + 8, y + 20 + l * 10, MUTED, false);
            }
            // toggle
            boolean on = m.isEnabled();
            int tw = 22, th = 10, tx2 = x + cw - tw - 8, ty = y + 7;
            Hud.round(c, tx2, ty, tw, th, on ? accent : 0xFF3A3A44);
            int kx = on ? tx2 + tw - th + 1 : tx2 + 1;
            c.fill(kx, ty + 1, kx + th - 2, ty + th - 1, 0xFFFFFFFF);
            // favourite star
            boolean fav = Config.get().favourites.contains(m.id);
            c.drawText(textRenderer, fav ? "★" : "☆", x + 8, y + chh - 13, fav ? 0xFFFFD166 : MUTED, false);
            if (inView) {
                final Module mod = m;
                hits.add(new Hit(x, y + chh - 16, 20, 16, () -> {
                    if (!Config.get().favourites.remove(mod.id)) Config.get().favourites.add(mod.id);
                    Config.save();
                }));
                hits.add(new Hit(x, y, cw, chh, () -> {
                    mod.setEnabled(!mod.isEnabled());
                    Config.save();
                }));
            }
        }
        c.disableScissor();
        if (list.isEmpty()) {
            String s = category.equals("Favourites") ? "Click the star on a mod to add it here." : "No mods match.";
            c.drawText(textRenderer, s, gx + 8, gy + 8, MUTED, false);
        }
    }

    private void drawSettings(DrawContext c, int mx, int my, int accent) {
        int x = px + 126, y = py + 42, w = pw - 126 - 14;
        Config cfg = Config.get();
        y = settingRow(c, x, y, w, "HUD size", String.format("%.1fx", cfg.scale), mx, my,
                () -> { cfg.scale = Math.max(0.5f, Math.round((cfg.scale - 0.1f) * 10) / 10f); Config.save(); },
                () -> { cfg.scale = Math.min(2.0f, Math.round((cfg.scale + 0.1f) * 10) / 10f); Config.save(); });
        y = settingToggle(c, x, y, w, "Text shadow", cfg.shadow, accent, () -> { cfg.shadow = !cfg.shadow; Config.save(); });
        y = settingToggle(c, x, y, w, "HUD background", cfg.background, accent, () -> { cfg.background = !cfg.background; Config.save(); });
        y = settingRow(c, x, y, w, "Accent colour", Config.ACCENT_NAMES[cfg.accent], mx, my,
                () -> { cfg.accent = (cfg.accent + Config.ACCENTS.length - 1) % Config.ACCENTS.length; Config.save(); },
                () -> { cfg.accent = (cfg.accent + 1) % Config.ACCENTS.length; Config.save(); });
        c.drawText(textRenderer, "Menu key: Right Shift (change it in Options > Controls).", x, y + 8, MUTED, false);
    }

    private int settingRow(DrawContext c, int x, int y, int w, String label, String value, int mx, int my, Runnable minus, Runnable plus) {
        Hud.round(c, x, y, w, 26, CARD);
        c.drawText(textRenderer, label, x + 10, y + 9, TEXT, false);
        int bx = x + w - 110;
        smallButton(c, bx, y + 4, "-", mx, my, minus);
        String v = value;
        c.drawText(textRenderer, v, bx + 18 + (60 - textRenderer.getWidth(v)) / 2, y + 9, TEXT, false);
        smallButton(c, bx + 84, y + 4, "+", mx, my, plus);
        return y + 32;
    }

    private void smallButton(DrawContext c, int x, int y, String s, int mx, int my, Runnable a) {
        boolean hov = mx >= x && mx < x + 18 && my >= y && my < y + 18;
        Hud.round(c, x, y, 18, 18, hov ? 0xFF33333D : 0xFF2A2A32);
        c.drawText(textRenderer, s, x + (18 - textRenderer.getWidth(s)) / 2, y + 5, TEXT, false);
        hits.add(new Hit(x, y, 18, 18, a));
    }

    private int settingToggle(DrawContext c, int x, int y, int w, String label, boolean on, int accent, Runnable a) {
        Hud.round(c, x, y, w, 26, CARD);
        c.drawText(textRenderer, label, x + 10, y + 9, TEXT, false);
        int tw = 22, th = 10, tx = x + w - tw - 10, ty = y + 8;
        Hud.round(c, tx, ty, tw, th, on ? accent : 0xFF3A3A44);
        int kx = on ? tx + tw - th + 1 : tx + 1;
        c.fill(kx, ty + 1, kx + th - 2, ty + th - 1, 0xFFFFFFFF);
        hits.add(new Hit(x, y, w, 26, a));
        return y + 32;
    }

    @Override
    public boolean mouseClicked(double mx, double my, int button) {
        if (super.mouseClicked(mx, my, button)) return true;
        if (button != 0) return false;
        // later hits were added on top, so test from the end
        for (int i = hits.size() - 1; i >= 0; i--) {
            Hit h = hits.get(i);
            if (h.in(mx, my)) {
                h.action.run();
                return true;
            }
        }
        return false;
    }

    // Both signatures exist on purpose: 1.20.1 has three parameters, later versions four.
    // No @Override, so the one that does not exist in a given version is just an unused method.
    public boolean mouseScrolled(double mx, double my, double vertical) {
        if (tab == Tab.MODS) {
            scroll -= (int) (vertical * 16);
            return true;
        }
        return false;
    }

    public boolean mouseScrolled(double mx, double my, double horizontal, double vertical) {
        return mouseScrolled(mx, my, vertical);
    }

    @Override
    public boolean keyPressed(int key, int scan, int mods) {
        if (key == GLFW.GLFW_KEY_RIGHT_SHIFT) {
            close();
            return true;
        }
        return super.keyPressed(key, scan, mods);
    }

    @Override
    public void close() {
        Config.save();
        super.close();
    }
}
