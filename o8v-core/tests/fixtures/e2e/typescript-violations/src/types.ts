// Shared types — intentionally wrong return type to cause cross-file errors

export interface User {
    id: number;
    name: string;
    email: string;
}

// Returns string but declared as returning number — callers will get type errors
export function getUserId(user: User): number {
    return user.name; // TS2322: string not assignable to number
}

// Wrong: should accept User but accepts string
export function formatUser(input: string): User {
    return input; // TS2322: string not assignable to User
}
