package dev.solocraft.client.mixin;

import dev.solocraft.client.Features;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Crosshair Editor: hides the vanilla crosshair while the custom one is on. */
@Mixin(InGameHud.class)
public abstract class InGameHudMixin {
    @Inject(method = "renderCrosshair", at = @At("HEAD"), cancellable = true)
    private void solocraft$crosshair(CallbackInfo ci) {
        if (Features.crosshairOn()) ci.cancel();
    }
}
