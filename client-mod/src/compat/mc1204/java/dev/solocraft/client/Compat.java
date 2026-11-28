package dev.solocraft.client;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.entity.effect.StatusEffectInstance;

/** Differences for Minecraft 1.20.2 - 1.20.4. */
public final class Compat {
    private Compat() {}

    public static String effectName(StatusEffectInstance e) {
        return e.getEffectType().getName().getString();
    }

    /** Screen.render already draws the background here. */
    public static void menuBackground(Screen s, DrawContext c) {}
}
