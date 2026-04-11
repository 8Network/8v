// Application entry point — imports from service.ts and types.ts

import { processUser, buildUser } from './service';
import { User } from './types';

// Unused variable — @typescript-eslint/no-unused-vars
const unusedConfig = { debug: true };

function run(): void {
    const user: User = {
        id: 1,
        name: "Alice",
        email: "alice@example.com"
    };

    // processUser returns string, assigned to number — cross-file type error
    const result: number = processUser(user); // TS2322

    // buildUser with a number instead of unknown
    const built = buildUser(42);
    console.log(result, built);
}

run();
