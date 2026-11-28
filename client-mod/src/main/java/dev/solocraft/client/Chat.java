package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.HashSet;
import java.util.Set;

/** Short messages in the game chat, so a problem is visible without opening log files. */
public final class Chat {
    private Chat() {}

    private static final Set<String> SHOWN = new HashSet<>();

    public static void info(String msg) {
        send("§a[SM Craft Client] §f" + msg);
    }

    /** Shown once per distinct message. */
    public static void error(String msg) {
        if (SHOWN.add(msg)) send("§c[SM Craft Client] " + msg);
    }

    private static void send(String s) {
        try {
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc.player != null) mc.player.sendMessage(Text.literal(s), false);
        } catch (Throwable ignored) {
            // chat is only a convenience
        }
    }

    public static String brief(Throwable t) {
        String m = t.getClass().getSimpleName() + (t.getMessage() == null ? "" : ": " + t.getMessage());
        return m.length() > 120 ? m.substring(0, 120) + "..." : m;
    }
}
