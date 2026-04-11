// Service layer — uses types from types.ts, triggers cross-file errors

import { User, getUserId, formatUser } from './types';

export function processUser(user: User): string {
    // getUserId returns number but we assign to string — TS2322 cross-file
    const id: string = getUserId(user); // TS2322: number not assignable to string
    return id;
}

export function buildUser(raw: unknown): User {
    // formatUser expects string but we pass unknown cast
    const result = formatUser(raw as string);
    // Unused variable triggers @typescript-eslint/no-unused-vars
    const unusedLocal = "never read";
    return result;
}

// Function with any type parameter — @typescript-eslint/no-explicit-any
export function dangerous(data: any): any {
    return data;
}
