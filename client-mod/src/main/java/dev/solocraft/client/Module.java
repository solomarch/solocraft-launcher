package dev.solocraft.client;

import net.minecraft.client.gui.DrawContext;

/** One feature in the menu. HUD modules also know how to draw themselves. */
public abstract class Module {
    public final String id;
    public final String name;
    public final String description;
    public final String category; // "HUD" or "Mechanic"
    public final boolean defaultOn;
    public final boolean hasHud;
    public final int defX, defY;

    /** Last drawn rectangle in screen pixels (used by the layout editor). */
    public int lastX, lastY, lastW, lastH;

    protected Module(String id, String name, String description, String category, boolean defaultOn,
                     boolean hasHud, int defX, int defY) {
        this.id = id;
        this.name = name;
        this.description = description;
        this.category = category;
        this.defaultOn = defaultOn;
        this.hasHud = hasHud;
        this.defX = defX;
        this.defY = defY;
    }

    public boolean isEnabled() {
        return Config.get().enabled.getOrDefault(id, defaultOn);
    }

    public void setEnabled(boolean on) {
        Config.get().enabled.put(id, on);
        onToggle(on);
    }

    public void onToggle(boolean on) {}

    /** Called every client tick while in a world. */
    public void tick(net.minecraft.client.MinecraftClient mc) {}

    public boolean hasSavedPos() {
        return Config.get().positions.containsKey(id);
    }

    public int x() {
        int[] p = Config.get().positions.get(id);
        return p != null ? p[0] : defX;
    }

    public int y() {
        int[] p = Config.get().positions.get(id);
        return p != null ? p[1] : defY;
    }

    public void moveTo(int x, int y) {
        Config.get().positions.put(id, new int[]{x, y});
    }

    /**
     * Draws the module with its top-left corner at (x, y) and returns {width, height}.
     * `editing` is true in the layout editor, where empty modules should still show a placeholder.
     */
    public int[] draw(DrawContext c, int x, int y, boolean editing) {
        return new int[]{0, 0};
    }
}
