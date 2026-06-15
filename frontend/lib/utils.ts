export type ClassValue = string | number | null | false | undefined | ClassValue[];

/**
 * Lightweight className combiner. Filters out falsy values and flattens arrays.
 * Intentionally dependency-free; variant maps in our primitives are authored so
 * classes don't conflict, so we don't need tailwind-merge here.
 */
export function cn(...inputs: ClassValue[]): string {
  const out: string[] = [];
  const walk = (value: ClassValue) => {
    if (value === null || value === undefined || value === false || value === '') return;
    if (Array.isArray(value)) {
      value.forEach(walk);
      return;
    }
    out.push(String(value));
  };
  inputs.forEach(walk);
  return out.join(' ');
}
