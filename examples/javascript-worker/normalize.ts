function trim(value: string): string {
    return value.trim();
}

export function normalize(value: string): string {
    return trim(value).toLowerCase();
}
