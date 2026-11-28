package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import org.lwjgl.glfw.GLFW;

/**
 * Reads the mouse and keyboard straight from the window. The menu screens use this instead of
 * overriding Screen.mouseClicked / keyPressed, whose parameters changed in Minecraft 1.21.9,
 * so the same screens work on every version.
 */
public final class Input {
    private Input() {}

    private static long handle() {
        MinecraftClient mc = MinecraftClient.getInstance();
        return mc.getWindow() == null ? 0L : mc.getWindow().getHandle();
    }

    public static boolean mouseDown(int button) {
        long h = handle();
        return h != 0 && GLFW.glfwGetMouseButton(h, button) == GLFW.GLFW_PRESS;
    }

    public static boolean keyDown(int key) {
        long h = handle();
        return h != 0 && GLFW.glfwGetKey(h, key) == GLFW.GLFW_PRESS;
    }

    /** With sticky buttons on, a very fast click is still seen by the next poll. */
    public static void sticky(boolean on) {
        long h = handle();
        if (h != 0) GLFW.glfwSetInputMode(h, GLFW.GLFW_STICKY_MOUSE_BUTTONS, on ? GLFW.GLFW_TRUE : GLFW.GLFW_FALSE);
    }

    /** Turns sticky on and clears any click left over from before the screen opened. */
    public static void beginScreen() {
        sticky(true);
        mouseDown(0);
        mouseDown(1);
    }

    /** Detects a button or key going down / up between frames. */
    public static final class Edge {
        private boolean prev;

        public Edge(boolean initial) { prev = initial; }

        public boolean rising(boolean now) {
            boolean r = now && !prev;
            prev = now;
            return r;
        }

        public boolean falling(boolean now) {
            boolean r = !now && prev;
            prev = now;
            return r;
        }

        public boolean held() { return prev; }
    }

    /** Types into a text by polling keys (letters, digits, space, minus, backspace). */
    public static final class Typing {
        private final boolean[] prev = new boolean[GLFW.GLFW_KEY_LAST + 1];
        private long backHeld;
        public final StringBuilder text = new StringBuilder();

        /** Call when the box gets focus so keys that are already down are not typed. */
        public void sync() {
            for (int k = GLFW.GLFW_KEY_SPACE; k <= GLFW.GLFW_KEY_LAST; k++) prev[k] = keyDown(k);
        }

        /** Returns true when the text changed. */
        public boolean update() {
            boolean changed = false;
            for (int k = GLFW.GLFW_KEY_SPACE; k <= GLFW.GLFW_KEY_Z; k++) {
                boolean d = keyDown(k);
                if (d && !prev[k] && text.length() < 40) {
                    if (k >= GLFW.GLFW_KEY_A && k <= GLFW.GLFW_KEY_Z) { text.append((char) ('a' + (k - GLFW.GLFW_KEY_A))); changed = true; }
                    else if (k >= GLFW.GLFW_KEY_0 && k <= GLFW.GLFW_KEY_9) { text.append((char) ('0' + (k - GLFW.GLFW_KEY_0))); changed = true; }
                    else if (k == GLFW.GLFW_KEY_SPACE) { text.append(' '); changed = true; }
                    else if (k == GLFW.GLFW_KEY_MINUS) { text.append('-'); changed = true; }
                }
                prev[k] = d;
            }
            boolean b = keyDown(GLFW.GLFW_KEY_BACKSPACE);
            long now = System.currentTimeMillis();
            if (b) {
                if (backHeld == 0) { backHeld = now; changed |= backspace(); }
                else if (now - backHeld > 450 && (now / 40) % 2 == 0) changed |= backspace();
            } else {
                backHeld = 0;
            }
            return changed;
        }

        private boolean backspace() {
            if (text.length() == 0) return false;
            text.setLength(text.length() - 1);
            return true;
        }
    }
}
