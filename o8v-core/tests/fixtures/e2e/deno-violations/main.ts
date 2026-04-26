// Main entry point — Deno-style relative imports

import { defaultPort, parseConfig, Config } from "./types.ts";

// Cross-file type error: defaultPort() returns number but declared as string
const port: string = defaultPort(); // TS2322: number not assignable to string

function start(config: Config): void {
    console.log(`Starting on ${config.host}:${config.port}`);
}

const cfg = parseConfig("{}");
start(cfg);
console.log(port);
