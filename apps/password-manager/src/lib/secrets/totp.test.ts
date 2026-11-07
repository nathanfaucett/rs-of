// @vitest-environment playwright
import { expect, it } from "vitest";
import { generateTotp, parseTotp } from "./totp";

const seed = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const seeds = {
    SHA1: seed,
    SHA256: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA",
    SHA512: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA",
};
// RFC 6238 Appendix B.
it.each([
    [59, "94287082", "46119246", "90693936"],
    [1111111109, "07081804", "68084774", "25091201"],
    [1111111111, "14050471", "67062674", "99943326"],
    [1234567890, "89005924", "91819424", "93441116"],
    [2000000000, "69279037", "90698825", "38618901"],
    [20000000000, "65353130", "77737706", "47863826"],
] as const)(
    "matches all RFC algorithms at %i seconds",
    async (time, sha1, sha256, sha512) => {
        for (const [index, algorithm] of (
            ["SHA1", "SHA256", "SHA512"] as const
        ).entries()) {
            const parameters = parseTotp(
                `otpauth://totp/Test?secret=${seeds[algorithm]}&algorithm=${algorithm}&digits=8`,
            );
            expect((await generateTotp(parameters, time * 1000)).code).toBe(
                [sha1, sha256, sha512][index],
            );
        }
    },
);

it("supports raw grouped lowercase/padded Base32 without rewriting input", () => {
    const input = "  mzxw 6===  ";
    expect(new TextDecoder().decode(parseTotp(input).secret)).toBe("foo");
    expect(input).toBe("  mzxw 6===  ");
    expect(parseTotp(seed)).toMatchObject({
        algorithm: "SHA-1",
        digits: 6,
        period: 30,
    });
});

it.each([6, 7, 8])(
    "supports %i digits with periods 1, 30, 60 and 86400",
    async (digits) => {
        for (const [period, time] of [
            [1, 1000],
            [30, 30000],
            [60, 60000],
            [86400, 86400000],
        ]) {
            for (const [algorithm, expected] of [
                ["SHA1", { 6: "287082", 7: "4287082", 8: "94287082" }],
                ["SHA256", { 6: "119246", 7: "6119246", 8: "46119246" }],
                ["SHA512", { 6: "693936", 7: "0693936", 8: "90693936" }],
            ] as const) {
                const parameters = parseTotp(
                    `otpauth://totp/Test?secret=${seeds[algorithm]}&algorithm=${algorithm}&digits=${digits}&period=${period}`,
                );
                // RFC 6238 time 59 uses counter 1; decimal truncation keeps the final digits.
                expect((await generateTotp(parameters, time)).code).toBe(
                    expected[digits as 6 | 7 | 8],
                );
            }
        }
    },
);

it("uses the current wall-clock boundary and full next-period countdown", async () => {
    const parameters = parseTotp(seed);
    expect(await generateTotp(parameters, 29999)).toEqual({
        code: "755224",
        remaining: 1,
        expiresAt: 30000,
    });
    expect(await generateTotp(parameters, 30000)).toEqual({
        code: "287082",
        remaining: 30,
        expiresAt: 60000,
    });
    expect(await generateTotp(parameters, 60000)).toEqual({
        code: "359152",
        remaining: 30,
        expiresAt: 90000,
    });
    for (const time of [-1, NaN, Infinity, 0.5])
        await expect(generateTotp(parameters, time)).rejects.toThrow(
            "Invalid TOTP time",
        );
});

it.each([
    "",
    "A",
    "ABC",
    "ABCDEF",
    "MZ",
    "MZXW6=",
    "MZXW6====",
    "MZXW6===A",
    "abc!",
    "00000000",
    "a".repeat(4097),
    "https://totp/test?secret=PRIVATE-KEY",
    "otpauth://hotp/Test?secret=PRIVATE-KEY&counter=1",
    "otpauth://totp/?secret=PRIVATE-KEY",
    "otpauth://totp/Test?secret=%QQ",
    "otpauth://totp/Test?secret=%FF",
    "otpauth://user@totp/Test?secret=PRIVATE-KEY",
    `otpauth://totp/Test?secret=${seed}#`,
    `otpauth://totp/Test?secret=${seed}\n&digits=6`,
    `otpauth://totp/Test?secret=${seed}&secret=${seed}`,
    `otpauth://totp/Test?secret=${seed}&issuer=A&issuer=B`,
    `otpauth://totp/Test?secret=${seed}&algorithm=MD5`,
    `otpauth://totp/Test?secret=${seed}&algorithm=`,
    `otpauth://totp/Test?secret=${seed}&digits=9`,
    `otpauth://totp/Test?secret=${seed}&digits=06`,
    ...["0", "-1", "1.5", "1e2", "01", "86401", "Infinity", ""].map(
        (period) => `otpauth://totp/Test?secret=${seed}&period=${period}`,
    ),
    `otpauth://totp/Test?secret=${seed}&counter=1`,
    "steam://PRIVATE-KEY",
])(
    "rejects malformed/unsupported input case %# without echoing input",
    (input) => {
        let message = "";
        try {
            parseTotp(input);
        } catch (error) {
            message = (error as Error).message;
        }
        expect(message).not.toBe("");
        expect(message).not.toContain(seed);
        expect(message).not.toContain("PRIVATE-KEY");
    },
);
