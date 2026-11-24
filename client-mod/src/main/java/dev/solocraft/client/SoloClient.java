package dev.solocraft.client;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class SoloClient implements ClientModInitializer {
    public static final Logger LOG = LoggerFactory.getLogger("solocraft-client");
    private static KeyBinding menuKey;

    @Override
    public void onInitializeClient() {
        Config.load();
        Modules.init();
        menuKey = KeyBindingHelper.registerKeyBinding(new KeyBinding(
                "key.solocraft.menu", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_RIGHT_SHIFT, "category.solocraft"));

        ClientTickEvents.END_CLIENT_TICK.register(mc -> {
            while (menuKey.wasPressed()) {
                if (mc.currentScreen == null) mc.setScreen(new MenuScreen());
            }
            if (mc.player != null) {
                for (Module m : Modules.ALL) m.tick(mc);
            }
        });

        HudRenderCallback.EVENT.register((ctx, tickCounter) -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc.player == null || mc.options.hudHidden) return;
            Tracker.update(mc);
            // The layout editor draws the modules itself.
            if (mc.currentScreen instanceof HudEditorScreen) return;
            Hud.drawAll(ctx, false);
        });
        LOG.info("SoloCraft Client loaded. Press Right Shift in game.");
    }
}
