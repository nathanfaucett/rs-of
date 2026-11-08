import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import * as v from "valibot";
import {
    LockSettingsSchema,
    DEFAULT_LOCK_SETTINGS,
    isBrowserSession,
    createInactivityLock,
    LOCK_TIMEOUT_OPTIONS,
} from "./lock";

function safeParse(schema: unknown, value: unknown) {
    try {
        // @ts-expect-error valibot schema type is opaque
        return { success: true, output: v.parse(schema, value) };
    } catch {
        return { success: false };
    }
}

describe("LockSettingsSchema", () => {
    it("accepts 0 (browser session)", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 0 });
        expect(result.success).toBe(true);
        if (result.success) expect(result.output.timeoutMinutes).toBe(0);
    });

    it("accepts 1 minute", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 1 });
        expect(result.success).toBe(true);
    });

    it("accepts 5 minutes (default)", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 5 });
        expect(result.success).toBe(true);
    });

    it("accepts 15 minutes", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 15 });
        expect(result.success).toBe(true);
    });

    it("accepts 30 minutes", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 30 });
        expect(result.success).toBe(true);
    });

    it("rejects 2 minutes", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 2 });
        expect(result.success).toBe(false);
    });

    it("rejects 10 minutes", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 10 });
        expect(result.success).toBe(false);
    });

    it("rejects negative values", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: -1 });
        expect(result.success).toBe(false);
    });

    it("rejects non-integers", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: 5.5 });
        expect(result.success).toBe(false);
    });

    it("rejects strings", () => {
        const result = safeParse(LockSettingsSchema, { timeoutMinutes: "5" });
        expect(result.success).toBe(false);
    });
});

describe("DEFAULT_LOCK_SETTINGS", () => {
    it("equals { timeoutMinutes: 5 }", () => {
        expect(DEFAULT_LOCK_SETTINGS).toEqual({ timeoutMinutes: 5 });
    });

    it("passes schema validation", () => {
        const result = safeParse(LockSettingsSchema, DEFAULT_LOCK_SETTINGS);
        expect(result.success).toBe(true);
    });
});

describe("isBrowserSession", () => {
    it("returns true for timeoutMinutes: 0", () => {
        expect(isBrowserSession({ timeoutMinutes: 0 })).toBe(true);
    });

    it("returns false for timeoutMinutes: 1", () => {
        expect(isBrowserSession({ timeoutMinutes: 1 })).toBe(false);
    });

    it("returns false for timeoutMinutes: 5", () => {
        expect(isBrowserSession({ timeoutMinutes: 5 })).toBe(false);
    });

    it("returns false for timeoutMinutes: 30", () => {
        expect(isBrowserSession({ timeoutMinutes: 30 })).toBe(false);
    });
});

describe("LOCK_TIMEOUT_OPTIONS", () => {
    it("contains browser session option first", () => {
        expect(LOCK_TIMEOUT_OPTIONS[0]).toEqual({ value: 0, label: "Browser session (lock on close)" });
    });

    it("contains all expected options in order", () => {
        expect(LOCK_TIMEOUT_OPTIONS).toEqual([
            { value: 0, label: "Browser session (lock on close)" },
            { value: 1, label: "1 minute" },
            { value: 5, label: "5 minutes" },
            { value: 15, label: "15 minutes" },
            { value: 30, label: "30 minutes" },
        ]);
    });

    it("all values pass schema validation", () => {
        for (const opt of LOCK_TIMEOUT_OPTIONS) {
            const result = safeParse(LockSettingsSchema, { timeoutMinutes: opt.value });
            expect(result.success).toBe(true);
        }
    });
});

describe("createInactivityLock", () => {
    let onLock: ReturnType<typeof vi.fn>;
    let lock: ReturnType<typeof createInactivityLock>;
    let currentTime: number;

    const timeProvider = () => currentTime;

    beforeEach(() => {
        onLock = vi.fn();
        lock = createInactivityLock(onLock, timeProvider);
        currentTime = 1000_000_000_000; // Fixed start time
    });

    afterEach(() => {
        lock.stop();
    });

    describe("browser session mode (timeoutMinutes: 0)", () => {
        it("never triggers onLock", () => {
            lock.start({ timeoutMinutes: 0 });
            currentTime += 60 * 60 * 1000; // 1 hour
            expect(onLock).not.toHaveBeenCalled();
        });

        it("check() returns false", () => {
            lock.start({ timeoutMinutes: 0 });
            expect(lock.check()).toBe(false);
            currentTime += 60 * 60 * 1000;
            expect(lock.check()).toBe(false);
        });

        it("activity() does nothing", () => {
            lock.start({ timeoutMinutes: 0 });
            lock.activity();
            currentTime += 60 * 60 * 1000;
            expect(onLock).not.toHaveBeenCalled();
        });

        it("switching from browser session to timed locks works", () => {
            lock.start({ timeoutMinutes: 0 });
            lock.start({ timeoutMinutes: 1 });
            currentTime += 60 * 1000;
            lock.check();
            expect(onLock).toHaveBeenCalledTimes(1);
        });

        it("switching from timed to browser session disables timer", () => {
            lock.start({ timeoutMinutes: 1 });
            lock.start({ timeoutMinutes: 0 });
            currentTime += 60 * 1000;
            expect(onLock).not.toHaveBeenCalled();
        });
    });

    describe("timed mode (timeoutMinutes > 0)", () => {
        it("triggers onLock after timeout when check() called", () => {
            lock.start({ timeoutMinutes: 5 });
            currentTime += 5 * 60 * 1000;
            lock.check();
            expect(onLock).toHaveBeenCalledTimes(1);
        });

        it("check() returns true when deadline passed", () => {
            lock.start({ timeoutMinutes: 5 });
            currentTime += 5 * 60 * 1000;
            expect(lock.check()).toBe(true);
        });

        it("check() returns false before deadline", () => {
            lock.start({ timeoutMinutes: 5 });
            currentTime += 4 * 60 * 1000;
            expect(lock.check()).toBe(false);
        });

        it("activity() resets deadline", () => {
            lock.start({ timeoutMinutes: 5 });
            currentTime += 4 * 60 * 1000;
            lock.activity();
            currentTime += 4 * 60 * 1000; // 8 min total, but only 4 since activity
            expect(lock.check()).toBe(false);
            currentTime += 60 * 1000; // 5 min since activity
            lock.check();
            expect(onLock).toHaveBeenCalledTimes(1);
        });

        it("stop() prevents onLock", () => {
            lock.start({ timeoutMinutes: 5 });
            lock.stop();
            currentTime += 10 * 60 * 1000;
            expect(onLock).not.toHaveBeenCalled();
        });

        it("calling start twice resets timer", () => {
            lock.start({ timeoutMinutes: 5 });
            currentTime += 4 * 60 * 1000;
            lock.start({ timeoutMinutes: 5 }); // reset
            currentTime += 4 * 60 * 1000; // 8 min total, but only 4 since second start
            expect(onLock).not.toHaveBeenCalled();
        });
    });
});
