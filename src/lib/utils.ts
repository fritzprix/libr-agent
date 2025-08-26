import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Throttle an async function so it can only be called once per interval.
 * If called again during the wait, only the last call will be executed after the interval.
 *
 * @param fn The async function to throttle
 * @param wait The minimum interval (ms) between calls
 * @returns A throttled version of the function
 */
export function throttlePromise<Args extends readonly unknown[], Return>(
  fn: (...args: Args) => Promise<Return>,
  wait: number,
): (...args: Args) => Promise<Return> {
  let lastCall = 0;
  let timeout: ReturnType<typeof setTimeout> | null = null;
  let pendingArgs: Args | null = null;
  let pendingResolve: ((value: Return) => void) | null = null;

  return (...args: Args): Promise<Return> => {
    const now = Date.now();
    return new Promise((resolve) => {
      const call = async () => {
        lastCall = Date.now();
        timeout = null;
        pendingArgs = null;
        pendingResolve = null;
        const result = await fn(...args);
        resolve(result);
      };
      if (now - lastCall >= wait) {
        call();
      } else {
        pendingArgs = args;
        pendingResolve = resolve;
        if (!timeout) {
          timeout = setTimeout(
            () => {
              if (pendingArgs && pendingResolve) {
                call();
              }
            },
            wait - (now - lastCall),
          );
        }
      }
    });
  };
}

export function toValidJsName(name: string): string {
  // Replace invalid characters with underscores
  let validName = name.replace(/[^a-zA-Z0-9_$]/g, '_');

  // If the name starts with a digit, prefix it with an underscore
  if (/^[0-9]/.test(validName)) {
    validName = '_' + validName;
  }

  // If the name is a reserved keyword, append an underscore
  const reservedKeywords = new Set([
    'break',
    'case',
    'catch',
    'class',
    'const',
    'continue',
    'debugger',
    'default',
    'delete',
    'do',
    'else',
    'export',
    'extends',
    'finally',
    'for',
    'function',
    'if',
    'import',
    'in',
    'instanceof',
    'new',
    'return',
    'super',
    'switch',
    'this',
    'throw',
    'try',
    'typeof',
    'var',
    'void',
    'while',
    'with',
    'yield',
    // Future reserved keywords
    'enum',
    'implements',
    'interface',
    'let',
    'package',
    'private',
    'protected',
    'public',
    'static',
    'await',
    // Literals
    'null',
    'true',
    'false',
  ]);

  if (reservedKeywords.has(validName)) {
    validName += '_';
  }

  return validName;
}
