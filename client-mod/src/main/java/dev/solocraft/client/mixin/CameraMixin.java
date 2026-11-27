package dev.solocraft.client.mixin;

import dev.solocraft.client.Features;
import net.minecraft.client.render.Camera;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/**
 * Free Look. The camera is orbited around the player's head: the free-look angles are used when the game places the
 * camera (so the third-person position follows them) and again at the end as a safety net.
 */
@Mixin(Camera.class)
public abstract class CameraMixin {
    @ModifyArgs(method = "update", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/Camera;setRotation(FF)V"))
    private void solocraft$angles(Args args) {
        if (Features.freeActive) {
            args.set(0, Features.freeYaw);
            args.set(1, Features.freePitch);
        }
    }

    @Inject(method = "update", at = @At("TAIL"))
    private void solocraft$freeLook(CallbackInfo ci) {
        Features.applyFreeLookCamera(this);
    }
}
