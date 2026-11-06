export interface TotpParameters {
    secret: Uint8Array<ArrayBuffer>;
    algorithm: "SHA-1" | "SHA-256" | "SHA-512";
    digits: 6 | 7 | 8;
    period: number;
}

export interface TotpCode {
    code: string;
    remaining: number;
    expiresAt: number;
}

export function parseTotp(input: string): TotpParameters {
    if (!input || input.length > 4096)
        throw new Error("Invalid authenticator key length.");
    let secret = input.trim();
    let algorithm: TotpParameters["algorithm"] = "SHA-1";
    let digits: TotpParameters["digits"] = 6;
    let period = 30;
    if (secret.includes(":")) {
        if (
            [...secret].some(
                (character) =>
                    character.charCodeAt(0) <= 32 ||
                    character.charCodeAt(0) === 127 ||
                    character === "#",
            )
        )
            throw new Error("Malformed authenticator URI.");
        let uri: URL;
        try {
            decodeURIComponent(secret);
            uri = new URL(secret);
        } catch {
            throw new Error("Malformed authenticator URI.");
        }
        if (
            !secret.startsWith("otpauth://totp/") ||
            uri.protocol !== "otpauth:" ||
            uri.hostname !== "totp" ||
            uri.username ||
            uri.password ||
            uri.port ||
            uri.hash ||
            uri.pathname.length < 2
        ) {
            throw new Error(
                "Only otpauth TOTP URIs with an account label are supported.",
            );
        }
        const allowed = new Set([
            "secret",
            "algorithm",
            "digits",
            "period",
            "issuer",
        ]);
        const seen = new Set<string>();
        for (const [name] of uri.searchParams) {
            if (!allowed.has(name) || seen.has(name))
                throw new Error(
                    "Unsupported or duplicate authenticator URI parameter.",
                );
            seen.add(name);
        }
        secret = uri.searchParams.get("secret") ?? "";
        const hash = uri.searchParams.get("algorithm") ?? "SHA1";
        if (hash !== "SHA1" && hash !== "SHA256" && hash !== "SHA512")
            throw new Error(
                "Unsupported TOTP algorithm. Use SHA1, SHA256, or SHA512.",
            );
        algorithm =
            hash === "SHA1"
                ? "SHA-1"
                : hash === "SHA256"
                  ? "SHA-256"
                  : "SHA-512";
        const count = uri.searchParams.get("digits") ?? "6";
        if (count !== "6" && count !== "7" && count !== "8")
            throw new Error("Unsupported TOTP digits. Use 6, 7, or 8.");
        digits = Number(count) as TotpParameters["digits"];
        const interval = uri.searchParams.get("period") ?? "30";
        if (!/^[1-9][0-9]{0,4}$/.test(interval) || Number(interval) > 86400)
            throw new Error(
                "Unsupported TOTP period. Use an integer from 1 to 86400 seconds.",
            );
        period = Number(interval);
    }
    const normalized = secret.replace(/ /g, "").toUpperCase();
    if (!/^[A-Z2-7]+={0,6}$/.test(normalized))
        throw new Error("Invalid Base32 authenticator key.");
    const unpadded = normalized.replace(/=+$/, "");
    const remainder = unpadded.length % 8;
    if (
        ![0, 2, 4, 5, 7].includes(remainder) ||
        (normalized.includes("=") &&
            (normalized.length % 8 !== 0 ||
                normalized.length - unpadded.length !== (8 - remainder) % 8))
    ) {
        throw new Error("Invalid Base32 authenticator key length or padding.");
    }
    const bytes = new Uint8Array(Math.floor((unpadded.length * 5) / 8));
    let bits = 0;
    let buffer = 0;
    let index = 0;
    for (const character of unpadded) {
        buffer =
            (buffer << 5) |
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".indexOf(character);
        bits += 5;
        if (bits >= 8) {
            bits -= 8;
            bytes[index++] = (buffer >>> bits) & 255;
        }
        buffer &= (1 << bits) - 1;
    }
    if (buffer !== 0) {
        bytes.fill(0);
        throw new Error("Invalid Base32 authenticator key trailing bits.");
    }
    return { secret: bytes, algorithm, digits, period };
}

export async function generateTotp(
    parameters: TotpParameters,
    now = Date.now(),
): Promise<TotpCode> {
    if (!Number.isSafeInteger(now) || now < 0)
        throw new Error("Invalid TOTP time.");
    const { secret, algorithm, digits, period } = parameters;
    const counter = Math.floor(now / 1000 / period);
    const message = new Uint8Array(8);
    new DataView(message.buffer).setBigUint64(0, BigInt(counter));
    const key = await crypto.subtle.importKey(
        "raw",
        secret,
        { name: "HMAC", hash: algorithm },
        false,
        ["sign"],
    );
    const digest = new Uint8Array(
        await crypto.subtle.sign("HMAC", key, message),
    );
    const offset = digest[digest.length - 1] & 15;
    const binary = new DataView(digest.buffer).getUint32(offset) & 0x7fffffff;
    digest.fill(0);
    const expiresAt = (counter + 1) * period * 1000;
    return {
        code: String(binary % 10 ** digits).padStart(digits, "0"),
        remaining: Math.ceil((expiresAt - now) / 1000),
        expiresAt,
    };
}
