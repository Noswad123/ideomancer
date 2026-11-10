// Syntax-only TypeScript example: no framework, configuration, or startup.
import { normalize as normalizeRecord } from './normalize';

export function prepareBatch(records: readonly string[]): string[] {
    const prepared: string[] = [];
    for (const record of records) {
        prepared.push(normalizeRecord(record));
    }
    return prepared;
}

export class Formatter {
    constructor(private readonly prefix: string) {}

    protected count: number = 0;
    #lastValue: string | undefined;

    public $format(value: string): string {
        this.count += 1;
        this.#lastValue = value;
        return `${this.prefix}${value}`;
    }

    public lastValue(): string | undefined {
        return this.#lastValue;
    }
}
