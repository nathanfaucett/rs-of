export function normalizeTimestamp(value: string): string {
    const match =
        /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.exec(
            value,
        );
    if (!match) throw new Error("Invalid timestamp");
    const [, year, month, day, hour, minute, second] = match.map(Number);
    const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
    const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][
        month - 1
    ];
    if (
        !days ||
        day < 1 ||
        day > days ||
        hour > 23 ||
        minute > 59 ||
        second > 59 ||
        !Number.isFinite(Date.parse(value))
    )
        throw new Error("Invalid timestamp");
    const fraction = (/\.(\d+)/.exec(value)?.[1] ?? "")
        .replace(/0+$/, "")
        .padEnd(3, "0");
    return new Date(value).toISOString().replace(/\.\d{3}Z$/, `.${fraction}Z`);
}
