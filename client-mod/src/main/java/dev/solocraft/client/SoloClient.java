package dev.solocraft.client;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import org.lwjgl.glfw.GLFW;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class SoloClient implements ClientModInitializer {
    public static final Logger LOG = LoggerFactory.getLogger("solocraft-client");
    private static KeyBinding menuKey;
    private static boolean keyWorks;
    private static boolean announced;
    private static boolean announcedEver;
    private static Input.Edge fallback;

    private static KeyBinding register(KeyBinding k) {
        if (k == null) return null;
        try {
            return KeyBindingHelper.registerKeyBinding(k);
        } catch (Throwable t) {
            LOG.error("Could not register key {}", k.getTranslationKey(), t);
            return null;
        }
    }

    @Override
    public void onInitializeClient() {
        Config.load();
        Modules.init();
        KeyBinding k = Gfx.newKey("key.solocraft.menu", GLFW.GLFW_KEY_RIGHT_SHIFT);
        if (k != null) {
            try {
                menuKey = KeyBindingHelper.registerKeyBinding(k);
            } catch (Throwable t) {
                LOG.error("Could not register the menu key", t);
            }
        }

        Features.zoomKey = register(Gfx.newKey("key.solocraft.zoom", GLFW.GLFW_KEY_C));
        Features.freeKey = register(Gfx.newKey("key.solocraft.freelook", GLFW.GLFW_KEY_LEFT_ALT));
        Features.perspectiveKey = register(Gfx.newKey("key.solocraft.perspective", GLFW.GLFW_KEY_V));
        Features.waypointKey = register(Gfx.newKey("key.solocraft.waypoint", GLFW.GLFW_KEY_X));
        net.fabricmc.fabric.api.event.player.AttackEntityCallback.EVENT.register((player, world, hand, entity, hitResult) -> {
            if (player == MinecraftClient.getInstance().player) Tracker.recordAttack();
            return net.minecraft.util.ActionResult.PASS;
        });

        ClientLifecycleEvents.CLIENT_STOPPING.register(mc -> {
            try {
                for (Module m : Modules.ALL) m.stop(mc);
                // Some mods leave a non-daemon thread behind, so the game window closes but the
                // Java process never exits and the launcher keeps showing "Running". Give the game
                // 12 seconds to shut down by itself, then end the process.
                Thread t = new Thread(() -> {
                    try {
                        Thread.sleep(12000);
                    } catch (InterruptedException ignored) {
                    }
                    Runtime.getRuntime().halt(0);
                }, "SoloCraft-exit-watchdog");
                t.setDaemon(true);
                t.start();
            } catch (Exception ignored) {
            }
        });
        ClientTickEvents.END_CLIENT_TICK.register(mc -> {
            try {
                if (mc.player == null) {
                    announced = false;
                } else if (!announced) {
                    announced = true;
                    if (!announcedEver) Chat.info("v" + version() + " loaded. Press Right Shift for the menu." + (menuKey == null
                            ? " (Controls entry unavailable on this version, reading the key directly.)" : ""));
                    announcedEver = true;
                }
                if (menuKey != null) {
                    while (menuKey.wasPressed()) {
                        keyWorks = true;
                        if (mc.currentScreen == null) openMenu(mc);
                    }
                }
                // Second way in: read Right Shift straight from the keyboard. It switches itself off
                // as soon as the normal key binding has worked once.
                if (!keyWorks && mc.player != null && mc.currentScreen == null) {
                    if (fallback == null) fallback = new Input.Edge(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT));
                    if (fallback.rising(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT))) openMenu(mc);
                } else if (fallback != null) {
                    fallback.rising(Input.keyDown(GLFW.GLFW_KEY_RIGHT_SHIFT));
                }
                if (mc.player != null) {
                    for (Module m : Modules.ALL) {
                        if (Hud.isBroken(m)) continue;
                        try {
                            m.tick(mc);
                        } catch (Throwable t) {
                            Hud.markBroken(m, t);
                        }
                    }
                }
            } catch (Throwable t) {
                LOG.error("SM Craft Client tick error", t);
                Chat.error("error: " + Chat.brief(t));
            }
        });

        HudRenderCallback.EVENT.register((ctx, tickCounter) -> {
            try {
                MinecraftClient mc = MinecraftClient.getInstance();
                FpsMetrics.capture(mc);
                if (mc.player == null || mc.options.hudHidden) return;
                Tracker.update(mc);
                // The layout editor draws the modules itself.
                if (mc.currentScreen instanceof HudEditorScreen) return;
                Hud.drawAll(ctx, false);
                if (Features.crosshairOn()) {
                    try {
                        Features.drawCrosshair(ctx);
                    } catch (Throwable t) {
                        for (Module m : Modules.ALL) if (m.id.equals("crosshair")) Hud.markBroken(m, t);
                    }
                }
            } catch (Throwable t) {
                LOG.error("SM Craft Client HUD error", t);
                Chat.error("HUD error: " + Chat.brief(t));
            }
        });
        LOG.info("SM Craft Client loaded (works on every Minecraft 1.21.x). Press Right Shift in game.");
    }

    private static String version() {
        try {
            return net.fabricmc.loader.api.FabricLoader.getInstance().getModContainer("solocraft-client")
                    .map(c -> c.getMetadata().getVersion().getFriendlyString()).orElse("?");
        } catch (Throwable t) {
            return "?";
        }
    }

    private static void openMenu(MinecraftClient mc) {
        try {
            mc.setScreen(new MenuScreen());
        } catch (Throwable t) {
            LOG.error("Could not open the menu", t);
            Chat.error("menu could not open: " + Chat.brief(t));
        }
    }
}
