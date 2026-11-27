package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayDeque;

/** Counts clicks per second by polling the mouse buttons once per frame. */
public final class Tracker {
    private static final ArrayDeque<Long> LEFT = new ArrayDeque<>();
    private static final ArrayDeque<Long> RIGHT = new ArrayDeque<>();
    private static boolean prevL, prevR;

    private Tracker() {}

    public static void update(MinecraftClient mc) {
        long now = System.currentTimeMillis();
        boolean l = false, r = false;
        if (mc.currentScreen == null && mc.getWindow() != null) {
            long h = mc.getWindow().getHandle();
            l = GLFW.glfwGetMouseButton(h, GLFW.GLFW_MOUSE_BUTTON_LEFT) == GLFW.GLFW_PRESS;
            r = GLFW.glfwGetMouseButton(h, GLFW.GLFW_MOUSE_BUTTON_RIGHT) == GLFW.GLFW_PRESS;
        }
        if (l && !prevL) LEFT.addLast(now);
        if (r && !prevR) RIGHT.addLast(now);
        prevL = l;
        prevR = r;
        trim(LEFT, now);
        trim(RIGHT, now);
    }

    private static void trim(ArrayDeque<Long> q, long now) {
        while (!q.isEmpty() && now - q.peekFirst() > 1000) q.pollFirst();
    }

    public static int left() { return LEFT.size(); }
    public static int right() { return RIGHT.size(); }
    public static boolean leftDown() { return prevL; }
    public static boolean rightDown() { return prevR; }
}
