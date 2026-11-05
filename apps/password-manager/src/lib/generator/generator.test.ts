import { afterEach, expect, it, vi } from "vitest";
import {
    generatePassword,
    generatePassphrase,
    validatePasswordSettings,
    validatePassphraseSettings,
} from "./generator";
import words from "./english.txt?raw";

it("uses all 2048 unique bundled words with independent replacement, case, separator and a random digit", async () => {
    const list = words.trim().split("\n");
    expect(list).toHaveLength(2048);
    expect(new Set(list).size).toBe(2048);
    const hash = await crypto.subtle.digest(
        "SHA-256",
        new TextEncoder().encode(words),
    );
    expect(
        [...new Uint8Array(hash)]
            .map((n) => n.toString(16).padStart(2, "0"))
            .join(""),
    ).toBe("2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda");
    const values = [0, 2047, 0, 1, 0xffffffff, 9];
    vi.spyOn(crypto, "getRandomValues").mockImplementation((array) => {
        (array as Uint32Array)[0] = values.shift()!;
        return array;
    });
    expect(
        generatePassphrase({
            words: 4,
            separator: "-",
            capitalize: true,
            appendNumber: true,
        }),
    ).toBe("Abandon-Zoo-Abandon-Ability9");
    expect(values).toEqual([]);
});

it("rejects invalid word counts, separators and flags without using the RNG", () => {
    const random = vi.spyOn(crypto, "getRandomValues");
    const settings = {
        words: 6,
        separator: " ",
        capitalize: false,
        appendNumber: false,
    };
    for (const words of [0, 3, 21, 6.5, NaN, Infinity, undefined, "6"]) {
        expect(() =>
            generatePassphrase({ ...settings, words } as never),
        ).toThrow("Word count");
    }
    expect(
        validatePassphraseSettings({ ...settings, separator: "secret-input" }),
    ).toBe("Select a supported separator.");
    expect(() =>
        generatePassphrase({ ...settings, capitalize: 1 } as never),
    ).toThrow("settings");
    expect(random).not.toHaveBeenCalled();
});

const password = {
    length: 8,
    lowercase: true,
    uppercase: false,
    digits: false,
    symbols: false,
    excludeAmbiguous: false,
};
afterEach(() => vi.restoreAllMocks());

it("validates integer bounds, booleans, and at least one character class before requesting randomness", () => {
    const random = vi.spyOn(crypto, "getRandomValues");
    for (const length of [0, 7, 129, 8.5, NaN, Infinity, undefined, "16"]) {
        expect(() =>
            generatePassword({ ...password, length } as never),
        ).toThrow("Password length");
    }
    expect(
        validatePasswordSettings({ ...password, lowercase: false }),
    ).toContain("Select");
    expect(() =>
        generatePassword({ ...password, digits: "yes" } as never),
    ).toThrow("settings");
    expect(random).not.toHaveBeenCalled();
});

it("rejects whole candidates missing a selected class rather than forcing biased positions", () => {
    const values = [...Array(8).fill(0), 0, 26, 52, 62, 1, 27, 53, 63];
    vi.spyOn(crypto, "getRandomValues").mockImplementation((array) => {
        (array as Uint32Array)[0] = values.shift()!;
        return array;
    });
    expect(
        generatePassword({
            ...password,
            uppercase: true,
            digits: true,
            symbols: true,
        }),
    ).toBe("aA0!bB1@");
    expect(values).toEqual([]);
});

it("removes ambiguous characters from every enabled class", () => {
    let next = 0;
    vi.spyOn(crypto, "getRandomValues").mockImplementation((array) => {
        (array as Uint32Array)[0] = next++;
        return array;
    });
    const result = generatePassword({
        ...password,
        length: 128,
        uppercase: true,
        digits: true,
        symbols: true,
        excludeAmbiguous: true,
    });
    expect(result).toHaveLength(128);
    expect(result).not.toMatch(/[Il1O0o]/);
    expect(result).toMatch(/[a-z]/);
    expect(result).toMatch(/[A-Z]/);
    expect(result).toMatch(/[2-9]/);
    expect(result).toMatch(/[!@#$%&*?+\-=_.]/);
});

it("rejects the uneven uint32 tail before mapping to the alphabet", () => {
    const values = [0xffffffff, 0, 1, 2, 3, 4, 5, 6, 25];
    vi.spyOn(crypto, "getRandomValues").mockImplementation((array) => {
        (array as Uint32Array)[0] = values.shift()!;
        return array;
    });
    expect(generatePassword(password)).toBe("abcdefgz");
    expect(values).toEqual([]);
});
