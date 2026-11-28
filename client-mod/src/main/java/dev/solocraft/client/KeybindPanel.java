package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.util.*;

/**
 * Keybind Manager: every key binding of the game and of all installed mods, on a clickable
 * keyboard or in a list, with conflicts highlighted and one-click fixes.
 * Only version-stable KeyBinding calls are used; the "owner" (category) of a binding is found by type.
 */
public final class KeybindPanel {
    public interface HitSink {
        void add(int x, int y, int w, int h, Runnable action);
    }

    private enum View { KEYBOARD, LIST }

    private static final int TEXT = 0xFFE6E6EA, MUTED = 0xFF8B8B96, CARD = 0xFF202027, CARD_HOVER = 0xFF26262E;
    private static final int RED = 0xFFEF4444, FREE = 0xFF17171C;
    private static final String UNKNOWN = "key.keyboard.unknown";
    private static final int[] PALETTE = {0xFF38BDF8, 0xFFC084FC, 0xFFFB923C, 0xFFF472B6, 0xFFFACC15, 0xFF2DD4BF, 0xFFA3E635, 0xFFFB7185};

    private View view = View.KEYBOARD;
    private boolean onlyConflicts;
    private int scroll;
    private String selected = "";           // translation key of the key picked in keyboard view

    // rebinding
    private KeyBinding listening;
    private boolean[] prevKeys = new boolean[GLFW.GLFW_KEY_LAST + 1];
    private boolean[] prevMouse = new boolean[8];
    private String pendingKey;              // key pressed that is already in use
    private KeyBinding pendingTarget;
    private String message = "";

    public boolean isListening() {
        return listening != null || pendingTarget != null;
    }

    /** One row per wheel step. */
    public void scroll(double v) {
        scroll += v > 0 ? -1 : 1;
    }

    // ---------- reading the game's bindings ----------

    private static List<KeyBinding> all() {
        KeyBinding[] arr = MinecraftClient.getInstance().options.allKeys;
        List<KeyBinding> l = new ArrayList<>(Arrays.asList(arr));
        l.sort(Comparator.comparing((KeyBinding k) -> owner(k)).thenComparing(KeybindPanel::label));
        return l;
    }

    private static String label(KeyBinding k) {
        return Text.translatable(k.getTranslationKey()).getString();
    }

    private static String keyOf(KeyBinding k) {
        if (k.getTranslationKey().startsWith("key.debug.")) return UNKNOWN; // F3 combos are not plain keys
        return k.isUnbound() ? UNKNOWN : k.getBoundKeyTranslationKey();
    }

    private static Map<String, List<KeyBinding>> byKey(List<KeyBinding> all) {
        Map<String, List<KeyBinding>> m = new HashMap<>();
        for (KeyBinding k : all) {
            String key = keyOf(k);
            if (key.equals(UNKNOWN)) continue;
            m.computeIfAbsent(key, x -> new ArrayList<>()).add(k);
        }
        return m;
    }

    private static int conflictCount(Map<String, List<KeyBinding>> m) {
        int n = 0;
        for (List<KeyBinding> l : m.values()) if (l.size() > 1) n++;
        return n;
    }

    // ---------- which mod owns a binding (found by type, works on every version) ----------

    private static boolean ownerReady;
    private static Method catMethod;
    private static Method catLabel;
    private static final Map<KeyBinding, String> OWNER_CACHE = new HashMap<>();

    private static void initOwner() {
        if (ownerReady) return;
        ownerReady = true;
        try {
            Class<?> catType = null;
            for (Constructor<?> k : KeyBinding.class.getConstructors()) {
                Class<?>[] p = k.getParameterTypes();
                if (p.length == 4 && p[0] == String.class && p[1] == InputUtil.Type.class && p[2] == int.class) catType = p[3];
            }
            KeyBinding probe = MinecraftClient.getInstance().options.forwardKey;
            for (Method m : KeyBinding.class.getMethods()) {
                if (m.getParameterCount() != 0 || java.lang.reflect.Modifier.isStatic(m.getModifiers())) continue;
                if (catType != null && catType != String.class && m.getReturnType() == catType) {
                    catMethod = m;
                    for (Method t : catType.getMethods()) {
                        if (t.getParameterCount() == 0 && t.getReturnType() == Text.class) catLabel = t;
                    }
                } else if (catType == String.class && m.getReturnType() == String.class) {
                    Object v = m.invoke(probe);
                    if (v instanceof String s && s.startsWith("key.categories")) catMethod = m;
                }
            }
        } catch (Throwable t) {
            SoloClient.LOG.warn("Keybind owners unavailable: {}", t.toString());
        }
    }

    private static final Set<String> VANILLA = Set.of("movement", "gameplay", "inventory", "multiplayer", "misc", "creative", "ui", "streaming", "spectator");

    static String owner(KeyBinding k) {
        String c = OWNER_CACHE.get(k);
        if (c != null) return c;
        String tk = k.getTranslationKey();
        String res = "Other";
        try {
            initOwner();
            if (tk.startsWith("key.solocraft")) {
                res = "SoloCraft";
            } else if (catMethod != null) {
                Object cat = catMethod.invoke(k);
                if (cat instanceof String s) {
                    String id = s.replace("key.categories.", "");
                    res = VANILLA.contains(id) ? "Minecraft" : prettify(id);
                } else if (cat != null) {
                    String id = String.valueOf(cat);
                    if (id.contains("minecraft")) {
                        res = "Minecraft";
                    } else if (catLabel != null) {
                        res = ((Text) catLabel.invoke(cat)).getString();
                    } else {
                        res = "Other";
                    }
                }
            }
        } catch (Throwable ignored) {
            // keep "Other"
        }
        OWNER_CACHE.put(k, res);
        return res;
    }

    private static String prettify(String id) {
        String s = id.contains(".") ? id.substring(id.lastIndexOf('.') + 1) : id;
        s = s.replace('_', ' ').replace('-', ' ');
        return s.isEmpty() ? "Other" : Character.toUpperCase(s.charAt(0)) + s.substring(1);
    }

    private static int colorOf(String owner) {
        if (owner.equals("Minecraft")) return 0xFF6B7280;
        if (owner.equals("SoloCraft")) return 0xFF4ADE80;
        return PALETTE[Math.floorMod(owner.hashCode(), PALETTE.length)];
    }

    // ---------- changing bindings ----------

    private static void setKey(KeyBinding k, String translationKey) {
        InputUtil.Key key = translationKey.equals(UNKNOWN) ? InputUtil.UNKNOWN_KEY : InputUtil.fromTranslationKey(translationKey);
        k.setBoundKey(key);
    }

    private static void saveAll() {
        try {
            KeyBinding.updateKeysByCode();
            MinecraftClient.getInstance().options.write();
        } catch (Throwable t) {
            SoloClient.LOG.warn("Could not save the key settings: {}", t.toString());
        }
    }

    private void apply(KeyBinding target, String newKey, boolean swap) {
        String old = keyOf(target);
        if (swap) {
            for (KeyBinding o : all()) {
                if (o != target && keyOf(o).equals(newKey)) setKey(o, old);
            }
        }
        setKey(target, newKey);
        saveAll();
        message = "'" + label(target) + "' is now " + target.getBoundKeyLocalizedText().getString();
    }

    private void fixConflicts() {
        List<KeyBinding> all = all();
        Map<String, List<KeyBinding>> m = byKey(all);
        Set<String> used = new HashSet<>(m.keySet());
        String[] candidates = freeCandidates();
        int moved = 0, ci = 0;
        for (List<KeyBinding> group : m.values()) {
            if (group.size() < 2) continue;
            // keep the vanilla / first binding where it is, move the others
            group.sort(Comparator.comparing((KeyBinding k) -> !owner(k).equals("Minecraft")));
            for (int i = 1; i < group.size(); i++) {
                while (ci < candidates.length && used.contains(candidates[ci])) ci++;
                if (ci >= candidates.length) break;
                setKey(group.get(i), candidates[ci]);
                used.add(candidates[ci]);
                moved++;
            }
        }
        saveAll();
        message = moved == 0 ? "No conflicts to fix." : "Moved " + moved + " binding" + (moved == 1 ? "" : "s") + " to free keys.";
    }

    private static String[] freeCandidates() {
        List<String> l = new ArrayList<>();
        for (char c : "hjklxvbnyuopzcg".toCharArray()) l.add("key.keyboard." + c);
        for (int i = 1; i <= 12; i++) l.add("key.keyboard.f" + i);
        for (int i = 0; i <= 9; i++) l.add("key.keyboard.keypad." + i);
        return l.toArray(new String[0]);
    }

    private void startListening(KeyBinding k) {
        listening = k;
        pendingTarget = null;
        pendingKey = null;
        Arrays.fill(prevKeys, true);   // ignore anything already held down
        Arrays.fill(prevMouse, true);
        for (int c = GLFW.GLFW_KEY_SPACE; c <= GLFW.GLFW_KEY_LAST; c++) prevKeys[c] = Input.keyDown(c);
        for (int b = 0; b < prevMouse.length; b++) prevMouse[b] = Input.mouseDown(b);
        message = "";
    }

    /** Watches the keyboard while waiting for a new key. */
    private void pollListening() {
        if (listening == null) return;
        if (Input.keyDown(GLFW.GLFW_KEY_ESCAPE) && !prevKeys[GLFW.GLFW_KEY_ESCAPE]) {
            listening = null;
            return;
        }
        String chosen = null;
        for (int c = GLFW.GLFW_KEY_SPACE; c <= GLFW.GLFW_KEY_LAST; c++) {
            boolean d = Input.keyDown(c);
            if (d && !prevKeys[c] && c != GLFW.GLFW_KEY_ESCAPE) {
                if (c == GLFW.GLFW_KEY_DELETE || c == GLFW.GLFW_KEY_BACKSPACE) chosen = UNKNOWN;
                else chosen = InputUtil.Type.KEYSYM.createFromCode(c).getTranslationKey();
            }
            prevKeys[c] = d;
        }
        // middle button and the two side buttons (left/right clicks are used by the menu itself)
        for (int b : new int[]{2, 3, 4}) {
            boolean d = Input.mouseDown(b);
            if (d && !prevMouse[b]) chosen = InputUtil.Type.MOUSE.createFromCode(b).getTranslationKey();
            prevMouse[b] = d;
        }
        if (chosen == null) return;
        KeyBinding target = listening;
        listening = null;
        if (!chosen.equals(UNKNOWN)) {
            List<String> users = new ArrayList<>();
            for (KeyBinding o : all()) if (o != target && keyOf(o).equals(chosen)) users.add(owner(o) + " - " + label(o));
            if (!users.isEmpty()) {
                pendingTarget = target;
                pendingKey = chosen;
                message = "Already used by: " + String.join(", ", users);
                return;
            }
        }
        apply(target, chosen, false);
    }

    // ---------- drawing ----------

    public void draw(DrawContext c, int x, int y, int w, int h, int mx, int my, String filter, HitSink realSink) {
        TextRenderer tr = Hud.tr();
        pollListening();
        // while a dialog is open, nothing behind it reacts to clicks
        final HitSink sink = isListening() ? (hx, hy, hw, hh, a) -> {} : realSink;
        List<KeyBinding> all;
        try {
            all = all();
        } catch (Throwable t) {
            Gfx.text(c, tr, "Key list unavailable on this version: " + Chat.brief(t), x, y + 6, RED, false);
            return;
        }
        Map<String, List<KeyBinding>> byKey = byKey(all);
        int conflicts = conflictCount(byKey);

        // toolbar
        int bx = x;
        bx = button(c, sink, tr, bx, y, "Keyboard", view == View.KEYBOARD, mx, my, () -> view = View.KEYBOARD);
        bx = button(c, sink, tr, bx, y, "List", view == View.LIST, mx, my, () -> { view = View.LIST; scroll = 0; });
        bx = button(c, sink, tr, bx, y, onlyConflicts ? "Showing conflicts" : "Conflicts only", onlyConflicts, mx, my,
                () -> { onlyConflicts = !onlyConflicts; scroll = 0; });
        if (conflicts > 0) {
            bx = button(c, sink, tr, bx, y, "Fix " + conflicts + " conflict" + (conflicts == 1 ? "" : "s"), false, mx, my, this::fixConflicts);
        }
        String status = all.size() + " keys, " + conflicts + (conflicts == 1 ? " conflict" : " conflicts");
        Gfx.text(c, tr, status, x + w - tr.getWidth(status), y + 5, conflicts > 0 ? RED : MUTED, false);
        if (!message.isEmpty() && pendingTarget == null) Gfx.text(c, tr, message, x, y + h - 10, MUTED, false);

        int by = y + 22, bh = h - 22 - (message.isEmpty() ? 0 : 12);
        if (view == View.KEYBOARD) drawKeyboard(c, x, by, w, bh, mx, my, byKey, sink);
        else drawList(c, x, by, w, bh, mx, my, all, byKey, filter, sink);

        if (listening != null) overlay(c, x, y, w, h, "Press a key for", label(listening), "Esc cancels, Delete unbinds", null, realSink);
        else if (pendingTarget != null) {
            final KeyBinding t = pendingTarget;
            final String k = pendingKey;
            overlay(c, x, y, w, h, "Key already in use", message.replace("Already used by: ", ""), "",
                    new String[]{"Swap the keys", "Use anyway", "Cancel"}, realSink,
                    new Runnable[]{() -> { apply(t, k, true); clearPending(); },
                                   () -> { apply(t, k, false); clearPending(); },
                                   this::clearPending});
        }
    }

    private void clearPending() {
        pendingTarget = null;
        pendingKey = null;
    }

    private int button(DrawContext c, HitSink sink, TextRenderer tr, int x, int y, String label, boolean on, int mx, int my, Runnable a) {
        int w = tr.getWidth(label) + 12;
        boolean hov = mx >= x && mx < x + w && my >= y && my < y + 16;
        Hud.round(c, x, y, w, 16, on ? Config.get().accentColor() & 0x00FFFFFF | 0x66000000 : hov ? CARD_HOVER : CARD);
        Gfx.text(c, tr, label, x + 6, y + 4, TEXT, false);
        sink.add(x, y, w, 16, a);
        return x + w + 4;
    }

    // keyboard layout: label, width in key units, translation key
    private record K(String label, float w, String tk) {}

    private static K k(String l, float w, String tk) { return new K(l, w, tk); }
    private static K ch(String l) { return new K(l, 1f, "key.keyboard." + l.toLowerCase()); }

    private static final K[][] LAYOUT = {
        {k("Esc", 1.4f, "key.keyboard.escape"), ch("F1"), ch("F2"), ch("F3"), ch("F4"), ch("F5"), ch("F6"), ch("F7"), ch("F8"), ch("F9"), ch("F10"), ch("F11"), ch("F12")},
        {k("`", 1, "key.keyboard.grave.accent"), ch("1"), ch("2"), ch("3"), ch("4"), ch("5"), ch("6"), ch("7"), ch("8"), ch("9"), ch("0"), k("-", 1, "key.keyboard.minus"), k("=", 1, "key.keyboard.equal"), k("Bksp", 1.6f, "key.keyboard.backspace")},
        {k("Tab", 1.5f, "key.keyboard.tab"), ch("Q"), ch("W"), ch("E"), ch("R"), ch("T"), ch("Y"), ch("U"), ch("I"), ch("O"), ch("P"), k("[", 1, "key.keyboard.left.bracket"), k("]", 1, "key.keyboard.right.bracket"), k("\\", 1.1f, "key.keyboard.backslash")},
        {k("Caps", 1.8f, "key.keyboard.caps.lock"), ch("A"), ch("S"), ch("D"), ch("F"), ch("G"), ch("H"), ch("J"), ch("K"), ch("L"), k(";", 1, "key.keyboard.semicolon"), k("'", 1, "key.keyboard.apostrophe"), k("Enter", 2f, "key.keyboard.enter")},
        {k("Shift", 2.3f, "key.keyboard.left.shift"), ch("Z"), ch("X"), ch("C"), ch("V"), ch("B"), ch("N"), ch("M"), k(",", 1, "key.keyboard.comma"), k(".", 1, "key.keyboard.period"), k("/", 1, "key.keyboard.slash"), k("R-Shift", 2.5f, "key.keyboard.right.shift")},
        {k("Ctrl", 1.4f, "key.keyboard.left.control"), k("Alt", 1.4f, "key.keyboard.left.alt"), k("Space", 6.2f, "key.keyboard.space"), k("R-Alt", 1.5f, "key.keyboard.right.alt"), k("R-Ctrl", 1.6f, "key.keyboard.right.control")},
        {k("Mouse L", 1.9f, "key.mouse.left"), k("Mouse M", 1.9f, "key.mouse.middle"), k("Mouse R", 1.9f, "key.mouse.right"), k("Side 1", 1.9f, "key.mouse.4"), k("Side 2", 1.9f, "key.mouse.5")},
    };

    private void drawKeyboard(DrawContext c, int x, int y, int w, int h, int mx, int my, Map<String, List<KeyBinding>> byKey, HitSink sink) {
        TextRenderer tr = Hud.tr();
        float unit = Math.min(26f, (w - 4) / 15.2f);
        int kh = 19, gap = 2;
        int cy = y;
        for (K[] row : LAYOUT) {
            float cx = x;
            for (K key : row) {
                int kw = Math.round(key.w * unit) - gap;
                int kx = Math.round(cx);
                List<KeyBinding> users = byKey.getOrDefault(key.tk, List.of());
                boolean conflict = users.size() > 1;
                int base = users.isEmpty() ? FREE : colorOf(owner(users.get(0)));
                int fill = conflict ? 0xFF7F1D1D : users.isEmpty() ? FREE : (base & 0x00FFFFFF) | 0x55000000;
                boolean sel = key.tk.equals(selected);
                boolean hov = mx >= kx && mx < kx + kw && my >= cy && my < cy + kh;
                Hud.round(c, kx, cy, kw, kh, hov ? (fill | 0x22000000) : fill);
                if (conflict) Gfx.border(c, kx, cy, kw, kh, RED);
                else if (sel) Gfx.border(c, kx, cy, kw, kh, 0xFFFBBF24);
                else if (!users.isEmpty()) Gfx.border(c, kx, cy, kw, kh, base);
                String l = key.label;
                while (l.length() > 1 && tr.getWidth(l) > kw - 3) l = l.substring(0, l.length() - 1);
                Gfx.text(c, tr, l, kx + (kw - tr.getWidth(l)) / 2, cy + 5, users.isEmpty() ? 0xFF6B6B76 : TEXT, false);
                final String tk = key.tk;
                sink.add(kx, cy, kw, kh, () -> selected = tk);
                cx += key.w * unit;
            }
            cy += kh + gap;
        }
        // details of the picked key
        cy += 4;
        if (selected.isEmpty()) {
            Gfx.text(c, tr, "Click a key to see what it does. Coloured = in use, red = conflict, dark = free.", x, cy + 2, MUTED, false);
            return;
        }
        List<KeyBinding> users = byKey.getOrDefault(selected, List.of());
        String name = Text.translatable(selected).getString();
        if (users.isEmpty()) {
            Gfx.text(c, tr, name + ": free key. Pick an action in List view, or click Change on any action below.", x, cy + 2, MUTED, false);
            return;
        }
        Gfx.text(c, tr, name + (users.size() > 1 ? "  -  CONFLICT: " + users.size() + " actions share it" : ""), x, cy + 2, users.size() > 1 ? RED : TEXT, false);
        cy += 13;
        for (KeyBinding kb : users) {
            if (cy + 14 > y + h) break;
            Gfx.text(c, tr, owner(kb), x + 2, cy + 3, colorOf(owner(kb)), false);
            Gfx.text(c, tr, label(kb), x + 92, cy + 3, TEXT, false);
            int bw = tr.getWidth("Change") + 12;
            Hud.round(c, x + w - bw - 2, cy, bw, 13, CARD);
            Gfx.text(c, tr, "Change", x + w - bw + 4, cy + 3, TEXT, false);
            sink.add(x + w - bw - 2, cy, bw, 13, () -> startListening(kb));
            cy += 14;
        }
    }

    private void drawList(DrawContext c, int x, int y, int w, int h, int mx, int my, List<KeyBinding> all,
                          Map<String, List<KeyBinding>> byKey, String filter, HitSink sink) {
        TextRenderer tr = Hud.tr();
        String q = filter == null ? "" : filter.trim().toLowerCase();
        List<Object> rows = new ArrayList<>();   // String = header, KeyBinding = row
        String last = null;
        for (KeyBinding kb : all) {
            String key = keyOf(kb);
            boolean conflict = !key.equals(UNKNOWN) && byKey.getOrDefault(key, List.of()).size() > 1;
            if (onlyConflicts && !conflict) continue;
            String o = owner(kb);
            if (!q.isEmpty() && !(label(kb).toLowerCase().contains(q) || o.toLowerCase().contains(q)
                    || kb.getBoundKeyLocalizedText().getString().toLowerCase().contains(q))) continue;
            if (!o.equals(last)) {
                rows.add(o);
                last = o;
            }
            rows.add(kb);
        }
        int rowH = 16;
        int vis = Math.max(1, h / rowH);
        scroll = Math.max(0, Math.min(scroll, Math.max(0, rows.size() - vis)));
        int cy = y;
        for (int ri = scroll; ri < Math.min(rows.size(), scroll + vis); ri++) {
            Object r = rows.get(ri);
            {
                if (r instanceof String o) {
                    Gfx.text(c, tr, o, x + 2, cy + 5, colorOf(o), true);
                } else {
                    KeyBinding kb = (KeyBinding) r;
                    String key = keyOf(kb);
                    boolean conflict = !key.equals(UNKNOWN) && byKey.getOrDefault(key, List.of()).size() > 1;
                    boolean inView = true;
                    boolean hov = mx >= x && mx < x + w && my >= cy && my < cy + rowH - 1;
                    Hud.round(c, x, cy, w, rowH - 1, hov ? CARD_HOVER : CARD);
                    String nm = label(kb);
                    while (nm.length() > 3 && tr.getWidth(nm) > w - 130) nm = nm.substring(0, nm.length() - 1);
                    Gfx.text(c, tr, nm, x + 6, cy + 4, TEXT, false);
                    String kt = kb.isUnbound() ? "-" : kb.getBoundKeyLocalizedText().getString();
                    int kw = Math.max(46, tr.getWidth(kt) + 10);
                    int kx = x + w - kw - 46;
                    Hud.round(c, kx, cy + 1, kw, rowH - 3, conflict ? 0xFF7F1D1D : 0xFF2A2A32);
                    if (conflict) Gfx.border(c, kx, cy + 1, kw, rowH - 3, RED);
                    Gfx.text(c, tr, kt, kx + (kw - tr.getWidth(kt)) / 2, cy + 4, TEXT, false);
                    String rs = "Reset";
                    int rx = x + w - 42;
                    Gfx.text(c, tr, rs, rx + 4, cy + 4, MUTED, false);
                    if (inView) {
                        sink.add(kx, cy, kw, rowH - 1, () -> startListening(kb));
                        sink.add(rx, cy, 40, rowH - 1, () -> {
                            setKey(kb, kb.getDefaultKey().getTranslationKey());
                            saveAll();
                        });
                    }
                }
            }
            cy += rowH;
        }
        if (rows.isEmpty()) Gfx.text(c, tr, onlyConflicts ? "No conflicts. Nice." : "No keys match.", x + 4, y + 6, MUTED, false);
    }

    private void overlay(DrawContext c, int x, int y, int w, int h, String title, String subject, String hint,
                         String[] buttons, HitSink sink, Runnable... actions) {
        TextRenderer tr = Hud.tr();
        int bw = Math.min(w - 20, 300), bh = 70;
        int bx = x + (w - bw) / 2, by = y + (h - bh) / 2;
        c.fill(x, y, x + w, y + h, 0xB0000000);
        Hud.round(c, bx, by, bw, bh, 0xFF1B1B21);
        Gfx.border(c, bx, by, bw, bh, Config.get().accentColor());
        Gfx.text(c, tr, title, bx + (bw - tr.getWidth(title)) / 2, by + 8, MUTED, false);
        String s = subject;
        while (s.length() > 3 && tr.getWidth(s) > bw - 16) s = s.substring(0, s.length() - 1);
        Gfx.text(c, tr, s, bx + (bw - tr.getWidth(s)) / 2, by + 22, TEXT, true);
        if (buttons == null) {
            Gfx.text(c, tr, hint, bx + (bw - tr.getWidth(hint)) / 2, by + 44, 0xFFFBBF24, false);
            return;
        }
        int cx = bx + 8;
        int each = (bw - 16 - 8) / 3;
        for (int i = 0; i < buttons.length; i++) {
            Hud.round(c, cx, by + 44, each, 16, i == 0 ? Config.get().accentColor() & 0x00FFFFFF | 0xAA000000 : CARD);
            Gfx.text(c, tr, buttons[i], cx + (each - tr.getWidth(buttons[i])) / 2, by + 48, TEXT, false);
            sink.add(cx, by + 44, each, 16, actions[i]);
            cx += each + 4;
        }
    }
}
