package dev.solocraft.client.mixin;

import dev.solocraft.client.Features;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Free Look: while active, mouse movement turns the camera instead of the player. */
@Mixin(Entity.class)
public abstract class EntityMixin {
    @Inject(method = "changeLookDirection", at = @At("HEAD"), cancellable = true)
    private void solocraft$freeLook(double dx, double dy, CallbackInfo ci) {
        if (Features.freeActive && (Object) this == MinecraftClient.getInstance().player) {
            Features.freeLookMouse(dx, dy);
            ci.cancel();
        }
    }
}
