import english from "./english.txt?raw";

const wordlist = english.trim().split("\n");

export interface PassphraseSettings {
    words: number;
    separator: string;
    capitalize: boolean;
    appendNumber: boolean;
}

export function validatePassphraseSettings(
    settings: PassphraseSettings,
): string | null {
    if (
        !Number.isInteger(settings.words) ||
        settings.words < 4 ||
        settings.words > 20
    )
        return "Word count must be a whole number from 4 to 20.";
    if (!["-", " ", ".", "_"].includes(settings.separator))
        return "Select a supported separator.";
    if (
        typeof settings.capitalize !== "boolean" ||
        typeof settings.appendNumber !== "boolean"
    )
        return "Invalid passphrase settings.";
    return null;
}

export function generatePassphrase(settings: PassphraseSettings): string {
    const error = validatePassphraseSettings(settings);
    if (error) throw new Error(error);
    const phrase = Array.from({ length: settings.words }, () => {
        const word = wordlist[randomIndex(wordlist.length)];
        return settings.capitalize
            ? word[0].toUpperCase() + word.slice(1)
            : word;
    }).join(settings.separator);
    return phrase + (settings.appendNumber ? randomIndex(10) : "");
}

export interface PasswordSettings {
    length: number;
    lowercase: boolean;
    uppercase: boolean;
    digits: boolean;
    symbols: boolean;
    excludeAmbiguous: boolean;
}

function randomIndex(size: number): number {
    const range = 0x100000000;
    const limit = range - (range % size);
    const buffer = new Uint32Array(1);
    try {
        do {
            crypto.getRandomValues(buffer);
        } while (buffer[0] >= limit);
        return buffer[0] % size;
    } finally {
        buffer.fill(0);
    }
}

function passwordClasses(settings: PasswordSettings): string[] {
    return [
        settings.lowercase ? "abcdefghijklmnopqrstuvwxyz" : "",
        settings.uppercase ? "ABCDEFGHIJKLMNOPQRSTUVWXYZ" : "",
        settings.digits ? "0123456789" : "",
        settings.symbols ? "!@#$%&*?+-=_." : "",
    ]
        .filter(Boolean)
        .map((group) =>
            settings.excludeAmbiguous ? group.replace(/[Il1O0o]/g, "") : group,
        );
}

export function validatePasswordSettings(
    settings: PasswordSettings,
): string | null {
    if (
        !Number.isInteger(settings.length) ||
        settings.length < 8 ||
        settings.length > 128
    )
        return "Password length must be a whole number from 8 to 128.";
    if (
        [
            settings.lowercase,
            settings.uppercase,
            settings.digits,
            settings.symbols,
            settings.excludeAmbiguous,
        ].some((value) => typeof value !== "boolean")
    )
        return "Invalid password settings.";
    if (!passwordClasses(settings).length)
        return "Select at least one character class.";
    return null;
}

export function generatePassword(settings: PasswordSettings): string {
    const error = validatePasswordSettings(settings);
    if (error) throw new Error(error);
    const groups = passwordClasses(settings);
    const alphabet = groups.join("");
    // Whole-candidate rejection keeps all strings that meet the class rules equally likely.
    for (;;) {
        const candidate = Array.from(
            { length: settings.length },
            () => alphabet[randomIndex(alphabet.length)],
        ).join("");
        if (
            groups.every((group) =>
                [...candidate].some((char) => group.includes(char)),
            )
        )
            return candidate;
    }
}
