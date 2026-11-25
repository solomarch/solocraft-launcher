package dev.solocraft.client;

import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.text.OrderedText;
import net.minecraft.util.Identifier;

import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

/**
 * The few Minecraft calls whose shape changed between 1.21 and 1.21.11, found at run time by
 * their TYPES (not their names), so one jar works on every 1.21.x:
 *  - DrawContext.getMatrices(): MatrixStack up to 1.21.5, JOML Matrix3x2fStack from 1.21.6.
 *  - KeyBinding constructor: String category up to 1.21.8, KeyBinding.Category from 1.21.9.
 * Anything that cannot be resolved is switched off instead of crashing the game.
 */
public final class Gfx {
    private Gfx() {}

    private static boolean ready, scaleOk;
    private static Method getMatrices;      // DrawContext -> matrix stack
    private static boolean legacy;          // true = MatrixStack
    private static Method pushM, popM, scaleM;

    private static void init() {
        if (ready) return;
        ready = true;
        try {
            for (Method m : DrawContext.class.getMethods()) {
                if (m.getParameterCount() == 0 && m.getReturnType() == MatrixStack.class) {
                    getMatrices = m;
                    legacy = true;
                }
            }
            if (getMatrices == null) {
                for (Method m : DrawContext.class.getMethods()) {
                    if (m.getParameterCount() == 0 && m.getReturnType().getName().equals("org.joml.Matrix3x2fStack")) {
                        getMatrices = m;
                        Class<?> k = m.getReturnType();
                        pushM = k.getMethod("pushMatrix");
                        popM = k.getMethod("popMatrix");
                        scaleM = k.getMethod("scale", float.class, float.class);
                    }
                }
            }
            scaleOk = getMatrices != null;
        } catch (Throwable t) {
            SoloClient.LOG.warn("HUD scaling is unavailable on this Minecraft version: {}", t.toString());
            scaleOk = false;
        }
        if (!scaleOk) SoloClient.LOG.warn("HUD size setting is ignored on this Minecraft version.");
    }

    public static boolean canScale() {
        init();
        return scaleOk;
    }

    /** Starts drawing at scale s. Call pop() afterwards (use try/finally). */
    public static void pushScale(DrawContext c, float s) {
        init();
        if (!scaleOk) return;
        try {
            Object st = getMatrices.invoke(c);
            if (legacy) {
                MatrixStack ms = (MatrixStack) st;
                ms.push();
                ms.scale(s, s, 1f);
            } else {
                pushM.invoke(st);
                scaleM.invoke(st, s, s);
            }
        } catch (Throwable t) {
            scaleOk = false;
            SoloClient.LOG.warn("HUD scaling switched off: {}", t.toString());
        }
    }

    public static void pop(DrawContext c) {
        init();
        if (!scaleOk) return;
        try {
            Object st = getMatrices.invoke(c);
            if (legacy) ((MatrixStack) st).pop();
            else popM.invoke(st);
        } catch (Throwable t) {
            scaleOk = false;
            SoloClient.LOG.warn("HUD scaling switched off: {}", t.toString());
        }
    }

    /** Creates a key binding on any version (the category argument changed type in 1.21.9). */
    public static KeyBinding newKey(String translationKey, int code) {
        try {
            Constructor<?> ctor = null;
            for (Constructor<?> k : KeyBinding.class.getConstructors()) {
                Class<?>[] p = k.getParameterTypes();
                if (p.length == 4 && p[0] == String.class && p[1] == InputUtil.Type.class && p[2] == int.class) ctor = k;
            }
            if (ctor == null) throw new IllegalStateException("no matching KeyBinding constructor");
            Class<?> cat = ctor.getParameterTypes()[3];
            Object catObj = null;
            if (cat == String.class) {
                catObj = "key.category.solocraft.main";
            } else {
                for (Method m : cat.getMethods()) {
                    Class<?>[] p = m.getParameterTypes();
                    if (Modifier.isStatic(m.getModifiers()) && p.length == 1 && p[0] == Identifier.class && m.getReturnType() == cat) {
                        catObj = m.invoke(null, Identifier.of("solocraft", "main"));
                        break;
                    }
                }
                if (catObj == null) {
                    for (var f : cat.getFields()) {
                        if (Modifier.isStatic(f.getModifiers()) && f.getType() == cat) { catObj = f.get(null); break; }
                    }
                }
            }
            if (catObj == null) throw new IllegalStateException("no key category available");
            return (KeyBinding) ctor.newInstance(translationKey, InputUtil.Type.KEYSYM, code, catObj);
        } catch (Throwable t) {
            SoloClient.LOG.error("Could not create the menu key: {}", t.toString());
            return null;
        }
    }

    // ---------- text ----------
    // DrawContext.drawText returned an int up to 1.21.5 and returns nothing from 1.21.6. The return type is part
    // of the method's signature, so a direct call breaks on one side. Found by parameter types instead.
    private static Method textString, textOrdered;
    private static boolean textInit;

    private static void initText() {
        if (textInit) return;
        textInit = true;
        for (Method m : DrawContext.class.getMethods()) {
            Class<?>[] p = m.getParameterTypes();
            if (p.length == 6 && p[0] == TextRenderer.class && p[2] == int.class && p[3] == int.class
                    && p[4] == int.class && p[5] == boolean.class) {
                if (p[1] == String.class) textString = m;
                else if (p[1] == OrderedText.class) textOrdered = m;
            }
        }
    }

    public static void border(DrawContext c, int x, int y, int w, int h, int color) {
        c.fill(x, y, x + w, y + 1, color);
        c.fill(x, y + h - 1, x + w, y + h, color);
        c.fill(x, y + 1, x + 1, y + h - 1, color);
        c.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    public static void text(DrawContext c, TextRenderer tr, String s, int x, int y, int color, boolean shadow) {
        initText();
        call(textString, c, tr, s, x, y, color, shadow);
    }

    public static void text(DrawContext c, TextRenderer tr, OrderedText s, int x, int y, int color, boolean shadow) {
        initText();
        call(textOrdered, c, tr, s, x, y, color, shadow);
    }

    private static void call(Method m, Object target, Object... args) {
        if (m == null) throw new IllegalStateException("text drawing is not available on this Minecraft version");
        try {
            m.invoke(target, args);
        } catch (InvocationTargetException e) {
            Throwable t = e.getCause() == null ? e : e.getCause();
            if (t instanceof RuntimeException re) throw re;
            throw new RuntimeException(t);
        } catch (IllegalAccessException e) {
            throw new RuntimeException(e);
        }
    }
}
