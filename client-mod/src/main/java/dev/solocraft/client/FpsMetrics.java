package dev.solocraft.client;

import net.minecraft.client.MinecraftClient;

import java.util.Arrays;

/** Lightweight rolling frame statistics used by SM Craft Client's FPS Companion integration. */
public final class FpsMetrics {
    private static final long WINDOW_NANOS = 10_000_000_000L;
    private static final int CAPACITY = 16_384;
    private static final long[] TIMES = new long[CAPACITY];
    private static final long[] STAMPS = new long[CAPACITY];
    private static int head, size, averageFps, onePercentLow;
    private static double averageMs, windowSeconds;
    private static long lastFrame, lastUpdate;

    private FpsMetrics() {}

    public static void capture(MinecraftClient mc) {
        if (mc.player == null) { lastFrame = 0; return; }
        long now = System.nanoTime();
        if (lastFrame != 0) {
            long elapsed = Math.max(1, now - lastFrame);
            if (size == CAPACITY) { head = (head + 1) % CAPACITY; size--; }
            int tail = (head + size) % CAPACITY;
            TIMES[tail] = elapsed;
            STAMPS[tail] = now;
            size++;
        }
        lastFrame = now;
        while (size > 0 && now - STAMPS[head] > WINDOW_NANOS) { head = (head + 1) % CAPACITY; size--; }
        if (lastUpdate == 0 || now - lastUpdate >= 1_000_000_000L) calculate(now);
    }

    private static void calculate(long now) {
        lastUpdate = now;
        if (size == 0) { averageFps = onePercentLow = 0; averageMs = windowSeconds = 0; return; }
        int newest = (head + size - 1) % CAPACITY;
        windowSeconds = Math.min(10, Math.max(0, STAMPS[newest] - STAMPS[head]) / 1_000_000_000.0);
        long total = 0;
        long[] sorted = new long[size];
        for (int i = 0; i < size; i++) { long t = TIMES[(head + i) % CAPACITY]; total += t; sorted[i] = t; }
        averageFps = (int) Math.round(size * 1_000_000_000.0 / Math.max(1, total));
        averageMs = total / (double) size / 1_000_000.0;
        Arrays.sort(sorted);
        int percentile = Math.min(size - 1, Math.max(0, (int) Math.ceil(size * 0.99) - 1));
        onePercentLow = (int) Math.round(1_000_000_000.0 / sorted[percentile]);
    }

    public static int averageFps() { return averageFps; }
    public static int onePercentLow() { return onePercentLow; }
    public static double averageMs() { return averageMs; }
    public static double windowSeconds() { return windowSeconds; }
    public static boolean hasFullWindow() { return windowSeconds >= 9.9; }
}
