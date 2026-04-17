export interface User {
  id: number;
  name: string;
  email?: string;
}

export function formatContact(user: User): string {
  const email = user.email;
  return `${user.name} <${email.toLowerCase()}>`;
}

export function head<T>(xs: readonly T[]): T | undefined {
  if (xs.length === 0) {
    return undefined;
  }
  return xs[0];
}
