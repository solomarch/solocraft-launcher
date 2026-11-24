package dev.solocraft.client;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.entity.effect.StatusEffectInstance;

/** Differences for Minecraft 1.20.1. */
public final class Compat {
    private Compat() {}

    public static String effectName(StatusEffectInstance e) {
        return e.getEffectType().getName().getString();
    }

    /** 1.20.1's Screen.render does not draw a background by itself. */
    public static void menuBackground(Screen s, DrawContext c) {
        s.renderBackground(c);
    }
}
