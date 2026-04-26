// Shared types for the Deno fixture

export interface Config {
    port: number;
    host: string;
    timeout: number;
}

// Returns wrong type — callers get TS2322
export function defaultPort(): number {
    return "8080"; // TS2322: string not assignable to number
}

export function parseConfig(raw: string): Config {
    return raw; // TS2322: string not assignable to Config
}
