package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.OrderedText;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/**
 * The Doki-style menu: sidebar, search, grid of module cards, keybinds, settings.
 * Everything is drawn by hand at an adjustable size and input is polled (see Input), so the same
 * screen works on every Minecraft 1.21.x.
 */
public class MenuScreen extends Screen {
    private static final int BASE_W = 540, BASE_H = 340;
    private static final int BG = 0xF0141418;
    private static final int PANEL = 0xFF1B1B21;
    private static final int CARD = 0xFF202027;
    private static final int CARD_HOVER = 0xFF26262E;
    private static final int TEXT = 0xFFE6E6EA;
    private static final int MUTED = 0xFF8B8B96;

    private enum Tab { MODS, SETTINGS, KEYBINDS }

    private Tab tab = Tab.MODS;
    private String category = "All";
    private final Input.Typing search = new Input.Typing();
    private boolean searchFocused;
    private int px, py, pw, ph;
    private int scrollRows;

    private final KeybindPanel keys = new KeybindPanel();
    private final Input.Edge mouse;
    private final Input.Edge shift;
    private boolean failed;
    private String lastKeybindError = "";

    // click targets rebuilt each frame (in menu units, not screen pixels)
    private record Hit(int x, int y, int w, int h, Runnable action) {
        boolean in(double mx, double my) { return mx >= x && mx < x + w && my >= y && my < y + h; }
    }
    private final List<Hit> hits = new ArrayList<>();

    public MenuScreen() {
        super(Text.literal("SM Craft Client"));
        Input.beginScreen();
        mouse = new Input.Edge(Input.mouseDown(0));
        shift = new Input.Edge(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT));
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public boolean shouldCloseOnEsc() {
        return !keys.isListening();
    }

    @Override
    public void removed() {
        Input.sticky(false);
        Config.save();
    }

    /** Menu size: the saved setting, shrunk when the window is too small to fit it. */
    private float scale() {
        if (!Gfx.canScale()) return 1f;
        float fit = Math.min((width - 8f) / BASE_W, (height - 8f) / BASE_H);
        return Math.max(0.6f, Math.min(Config.get().menuScale, fit));
    }

    private List<Module> visible() {
        String q = search.text.toString().trim().toLowerCase();
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
        try {
            renderMenu(c, mx, my, delta);
        } catch (Throwable t) {
            SoloClient.LOG.error("Menu error", t);
            Chat.error("menu error, closing it: " + Chat.brief(t));
            if (!failed) {
                failed = true;
                MinecraftClient.getInstance().execute(this::close);
            }
        }
    }

    private void renderMenu(DrawContext c, int rawX, int rawY, float delta) {
        Input.sticky(true); // re-armed every frame: the previous screen switches it off when it closes
        float s = scale();
        int lw = Math.round(width / s), lh = Math.round(height / s);
        pw = Math.min(lw - 8, BASE_W);
        ph = Math.min(lh - 8, BASE_H);
        px = (lw - pw) / 2;
        py = (lh - ph) / 2;
        int mx = Math.round(rawX / s), my = Math.round(rawY / s);
        boolean searchable = tab == Tab.MODS || tab == Tab.KEYBINDS;

        // ----- input -----
        boolean shiftNow = shift.rising(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT));
        if (shiftNow && !keys.isListening()) {
            close();
            return;
        }
        if (mouse.rising(Input.mouseDown(0))) {
            boolean wasFocused = searchFocused;
            searchFocused = false;
            for (int i = hits.size() - 1; i >= 0; i--) {
                Hit h = hits.get(i);
                if (h.in(mx, my)) {
                    h.action.run();
                    break;
                }
            }
            if (searchFocused && !wasFocused) search.sync();
        }
        if (searchFocused && searchable && !keys.isListening()) {
            if (search.update()) scrollRows = 0;
        }

        super.render(c, rawX, rawY, delta); // dimmed / blurred background
        hits.clear();
        Gfx.pushScale(c, s);
        try {
            drawMenu(c, mx, my);
        } finally {
            Gfx.pop(c);
        }
    }

    private void drawMenu(DrawContext c, int mx, int my) {
        TextRenderer tr = Hud.tr();
        int accent = Config.get().accentColor();
        boolean searchable = tab == Tab.MODS || tab == Tab.KEYBINDS;

        Hud.round(c, px, py, pw, ph, BG);
        Gfx.text(c, tr, "SM Craft Client", px + 12, py + 11, TEXT, true);
        int tx = px + 140;
        tx = tabButton(c, tx, py + 6, "Mods", Tab.MODS, mx, my);
        tx = tabButton(c, tx, py + 6, "Keybinds", Tab.KEYBINDS, mx, my);
        tx = tabButton(c, tx, py + 6, "Settings", Tab.SETTINGS, mx, my);
        int cx = px + pw - 18;
        Gfx.text(c, tr, "x", cx, py + 11, MUTED, false);
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
            Gfx.text(c, tr, labels[i], bx + 8, sy + 6, on ? TEXT : MUTED, false);
            String n = String.valueOf(count(cat));
            Gfx.text(c, tr, n, bx + bw - 8 - tr.getWidth(n), sy + 6, MUTED, false);
            hits.add(new Hit(bx, sy, bw, 20, () -> { tab = Tab.MODS; category = cat; scrollRows = 0; }));
            sy += 24;
        }
        // edit layout button
        int ey = py + ph - 32;
        boolean eh = mx >= px + 8 && mx < px + 112 && my >= ey && my < ey + 22;
        Hud.round(c, px + 8, ey, 104, 22, eh ? CARD_HOVER : CARD);
        String el = "Edit HUD Layout";
        Gfx.text(c, tr, el, px + 8 + (104 - tr.getWidth(el)) / 2, ey + 7, TEXT, false);
        hits.add(new Hit(px + 8, ey, 104, 22, () -> MinecraftClient.getInstance().setScreen(new HudEditorScreen(this))));

        // search box (mods and keybinds)
        if (searchable) drawSearch(c, tr, accent);

        if (tab == Tab.MODS) drawMods(c, tr, mx, my, accent);
        else if (tab == Tab.KEYBINDS) {
            // an error inside this tab is shown inside the tab, it never closes the whole menu
            try {
                keys.draw(c, px + 126, py + 62, pw - 126 - 14, ph - 62 - 10, mx, my, search.text.toString(),
                        (hx, hy, hw, hh, a) -> hits.add(new Hit(hx, hy, hw, hh, a)));
            } catch (Throwable t) {
                String msg = Chat.brief(t);
                if (!msg.equals(lastKeybindError)) {
                    lastKeybindError = msg;
                    SoloClient.LOG.error("Keybind Manager error", t);
                }
                int ex = px + 126, ey2 = py + 66, ew = pw - 126 - 14;
                Gfx.text(c, tr, "The Keybind Manager hit an error on this Minecraft version:", ex, ey2, 0xFFEF4444, false);
                int yy = ey2 + 14;
                for (OrderedText line : tr.wrapLines(Text.literal(msg), ew)) {
                    Gfx.text(c, tr, line, ex, yy, TEXT, false);
                    yy += 11;
                }
                Gfx.text(c, tr, "Details are in the game log. Everything else still works.", ex, yy + 6, MUTED, false);
            }
        } else drawSettings(c, tr, mx, my, accent);
    }

    private void drawSearch(DrawContext c, TextRenderer tr, int accent) {
        int x = px + 126, y = py + 38, w = pw - 126 - 14, h = 18;
        Hud.round(c, x, y, w, h, 0xFF101014);
        if (searchFocused) Gfx.border(c, x, y, w, h, accent);
        String t = search.text.toString();
        if (t.isEmpty() && !searchFocused) {
            Gfx.text(c, tr, "Search...", x + 6, y + 5, 0xFF55555E, false);
        } else {
            boolean caret = searchFocused && (System.currentTimeMillis() / 500) % 2 == 0;
            Gfx.text(c, tr, t + (caret ? "_" : ""), x + 6, y + 5, TEXT, false);
        }
        hits.add(new Hit(x, y, w, h, () -> searchFocused = true));
    }

    private int tabButton(DrawContext c, int x, int y, String label, Tab t, int mx, int my) {
        TextRenderer tr = Hud.tr();
        int w = tr.getWidth(label) + 20;
        boolean on = tab == t;
        boolean hov = mx >= x && mx < x + w && my >= y && my < y + 18;
        if (on) Hud.round(c, x, y, w, 18, CARD);
        else if (hov) Hud.round(c, x, y, w, 18, 0xFF18181D);
        Gfx.text(c, tr, label, x + 10, y + 5, on ? TEXT : MUTED, false);
        hits.add(new Hit(x, y, w, 18, () -> { tab = t; scrollRows = 0; }));
        return x + w + 4;
    }

    private void drawMods(DrawContext c, TextRenderer tr, int mx, int my, int accent) {
        int gx = px + 126, gy = py + 62, gw = pw - 126 - 14, gh = ph - 62 - 10;
        int cols = gw >= 360 ? 3 : 2;
        int gap = 8;
        int cw = (gw - gap * (cols - 1)) / cols;
        int chh = 54;
        List<Module> list = visible();
        int rows = (list.size() + cols - 1) / cols;
        int visRows = Math.max(1, (gh + gap) / (chh + gap));
        scrollRows = Math.max(0, Math.min(scrollRows, Math.max(0, rows - visRows)));
        for (int i = 0; i < list.size(); i++) {
            int row = i / cols - scrollRows;
            if (row < 0 || row >= visRows) continue; // scrolls a whole row at a time, so nothing is cut off
            Module m = list.get(i);
            int x = gx + (i % cols) * (cw + gap);
            int y = gy + row * (chh + gap);
            boolean hov = mx >= x && mx < x + cw && my >= y && my < y + chh;
            Hud.round(c, x, y, cw, chh, hov ? CARD_HOVER : CARD);
            Gfx.text(c, tr, m.name, x + 8, y + 7, TEXT, false);
            List<OrderedText> lines = tr.wrapLines(Text.literal(m.description), cw - 16);
            for (int l = 0; l < Math.min(2, lines.size()); l++) {
                Gfx.text(c, tr, lines.get(l), x + 8, y + 20 + l * 10, MUTED, false);
            }
            boolean on = m.isEnabled();
            int tw = 22, th = 10, tx2 = x + cw - tw - 8, ty = y + 7;
            Hud.round(c, tx2, ty, tw, th, on ? accent : 0xFF3A3A44);
            int kx = on ? tx2 + tw - th + 1 : tx2 + 1;
            c.fill(kx, ty + 1, kx + th - 2, ty + th - 1, 0xFFFFFFFF);
            boolean fav = Config.get().favourites.contains(m.id);
            Gfx.text(c, tr, fav ? "★" : "☆", x + 8, y + chh - 13, fav ? 0xFFFFD166 : MUTED, false);
            final Module mod = m;
            hits.add(new Hit(x, y, cw, chh, () -> {
                mod.setEnabled(!mod.isEnabled());
                Config.save();
            }));
            hits.add(new Hit(x, y + chh - 16, 20, 16, () -> {
                if (!Config.get().favourites.remove(mod.id)) Config.get().favourites.add(mod.id);
                Config.save();
            }));
        }
        if (list.isEmpty()) {
            String s = category.equals("Favourites") ? "Click the star on a mod to add it here." : "No mods match.";
            Gfx.text(c, tr, s, gx + 8, gy + 8, MUTED, false);
        } else if (rows > visRows) {
            String s = "Scroll for more (" + (scrollRows + 1) + "/" + (rows - visRows + 1) + ")";
            Gfx.text(c, tr, s, gx + gw - tr.getWidth(s), py + ph - 10, MUTED, false);
        }
    }

    private void drawSettings(DrawContext c, TextRenderer tr, int mx, int my, int accent) {
        int x = px + 126, y = py + 42, w = pw - 126 - 14;
        Config cfg = Config.get();
        y = settingRow(c, tr, x, y, w, "Menu size", String.format("%.1fx", cfg.menuScale), mx, my,
                () -> { cfg.menuScale = Math.max(0.8f, Math.round((cfg.menuScale - 0.1f) * 10) / 10f); Config.save(); },
                () -> { cfg.menuScale = Math.min(2.5f, Math.round((cfg.menuScale + 0.1f) * 10) / 10f); Config.save(); });
        y = settingRow(c, tr, x, y, w, "HUD size", String.format("%.1fx", cfg.scale), mx, my,
                () -> { cfg.scale = Math.max(0.5f, Math.round((cfg.scale - 0.1f) * 10) / 10f); Config.save(); },
                () -> { cfg.scale = Math.min(2.0f, Math.round((cfg.scale + 0.1f) * 10) / 10f); Config.save(); });
        y = settingToggle(c, tr, x, y, w, "Text shadow", cfg.shadow, accent, () -> { cfg.shadow = !cfg.shadow; Config.save(); });
        y = settingToggle(c, tr, x, y, w, "HUD background", cfg.background, accent, () -> { cfg.background = !cfg.background; Config.save(); });
        y = settingRow(c, tr, x, y, w, "Accent colour", Config.ACCENT_NAMES[cfg.accent], mx, my,
                () -> { cfg.accent = (cfg.accent + Config.ACCENTS.length - 1) % Config.ACCENTS.length; Config.save(); },
                () -> { cfg.accent = (cfg.accent + 1) % Config.ACCENTS.length; Config.save(); });
        y = settingRow(c, tr, x, y, w, "Zoom level", cfg.zoomLevel + "x", mx, my,
                () -> { cfg.zoomLevel = Math.max(2, cfg.zoomLevel - 1); Config.save(); },
                () -> { cfg.zoomLevel = Math.min(10, cfg.zoomLevel + 1); Config.save(); });
        int ns = Features.CROSS_STYLES.length, nc = Features.CROSS_COLORS.length;
        y = settingRow(c, tr, x, y, w, "Crosshair shape", Features.CROSS_STYLES[cfg.crossStyle], mx, my,
                () -> { cfg.crossStyle = (cfg.crossStyle + ns - 1) % ns; Config.save(); },
                () -> { cfg.crossStyle = (cfg.crossStyle + 1) % ns; Config.save(); });
        y = settingRow(c, tr, x, y, w, "Crosshair colour", Features.CROSS_COLORS[cfg.crossColor], mx, my,
                () -> { cfg.crossColor = (cfg.crossColor + nc - 1) % nc; Config.save(); },
                () -> { cfg.crossColor = (cfg.crossColor + 1) % nc; Config.save(); });
        y = settingRow(c, tr, x, y, w, "Crosshair size", String.valueOf(cfg.crossSize), mx, my,
                () -> { cfg.crossSize = Math.max(2, cfg.crossSize - 1); Config.save(); },
                () -> { cfg.crossSize = Math.min(14, cfg.crossSize + 1); Config.save(); });
        Gfx.text(c, tr, "Menu key: Right Shift (change it in the Keybinds tab).", x, y + 4, MUTED, false);
    }

    private int settingRow(DrawContext c, TextRenderer tr, int x, int y, int w, String label, String value, int mx, int my, Runnable minus, Runnable plus) {
        Hud.round(c, x, y, w, 26, CARD);
        Gfx.text(c, tr, label, x + 10, y + 9, TEXT, false);
        int bx = x + w - 110;
        smallButton(c, tr, bx, y + 4, "-", mx, my, minus);
        Gfx.text(c, tr, value, bx + 18 + (60 - tr.getWidth(value)) / 2, y + 9, TEXT, false);
        smallButton(c, tr, bx + 84, y + 4, "+", mx, my, plus);
        return y + 30;
    }

    private void smallButton(DrawContext c, TextRenderer tr, int x, int y, String s, int mx, int my, Runnable a) {
        boolean hov = mx >= x && mx < x + 18 && my >= y && my < y + 18;
        Hud.round(c, x, y, 18, 18, hov ? 0xFF33333D : 0xFF2A2A32);
        Gfx.text(c, tr, s, x + (18 - tr.getWidth(s)) / 2, y + 5, TEXT, false);
        hits.add(new Hit(x, y, 18, 18, a));
    }

    private int settingToggle(DrawContext c, TextRenderer tr, int x, int y, int w, String label, boolean on, int accent, Runnable a) {
        Hud.round(c, x, y, w, 26, CARD);
        Gfx.text(c, tr, label, x + 10, y + 9, TEXT, false);
        int tw = 22, th = 10, tx = x + w - tw - 10, ty = y + 8;
        Hud.round(c, tx, ty, tw, th, on ? accent : 0xFF3A3A44);
        int kx = on ? tx + tw - th + 1 : tx + 1;
        c.fill(kx, ty + 1, kx + th - 2, ty + th - 1, 0xFFFFFFFF);
        hits.add(new Hit(x, y, w, 26, a));
        return y + 30;
    }

    // Both signatures exist on purpose: 1.20.1 has three parameters, later versions four.
    // No @Override, so the one that does not exist in a given version is just an unused method.
    public boolean mouseScrolled(double mx, double my, double vertical) {
        int step = vertical > 0 ? -1 : 1;
        if (tab == Tab.KEYBINDS) {
            keys.scroll(vertical);
            return true;
        }
        if (tab == Tab.MODS) {
            scrollRows += step;
            return true;
        }
        return false;
    }

    public boolean mouseScrolled(double mx, double my, double horizontal, double vertical) {
        return mouseScrolled(mx, my, vertical);
    }

    @Override
    public void close() {
        Config.save();
        super.close();
    }
}
