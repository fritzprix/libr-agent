import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { BUILTIN_SERVICE_NAMES } from './tool-call-utils';

/**
 * Converts a string to an array of MCPContent objects with type 'text'.
 * This is a utility function to easily wrap plain text in the expected
 * format for certain components or functions that handle MCPContent.
 *
 * @param text The input string to convert.
 * @returns An array containing a single MCPContent object of type 'text'.
 */
export function stringToMCPContentArray(
  text: string,
): { type: 'text'; text: string }[] {
  return [{ type: 'text', text }];
}

/**
 * A utility function to merge Tailwind CSS classes.
 * It combines the functionalities of `clsx` and `tailwind-merge`.
 * `clsx` allows for conditional class names, and `tailwind-merge`
 * intelligently merges Tailwind CSS classes without conflicts.
 *
 * @param ...inputs The class values to merge. These can be strings, arrays, or objects.
 * @returns A string of merged class names.
 */
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
  let pendingResolvers: Array<(value: Return) => void> = [];

  return (...args: Args): Promise<Return> => {
    const now = Date.now();
    return new Promise((resolve) => {
      const call = async () => {
        lastCall = Date.now();
        timeout = null;

        const argsToUse = pendingArgs || args;
        const resolversToUse =
          pendingResolvers.length > 0 ? pendingResolvers : [resolve];

        pendingArgs = null;
        pendingResolvers = [];

        const result = await fn(...argsToUse);
        // Resolve all pending promises with the same result
        resolversToUse.forEach((r) => r(result));
      };
      if (now - lastCall >= wait) {
        call();
      } else {
        pendingArgs = args;
        pendingResolvers.push(resolve);
        if (!timeout) {
          timeout = setTimeout(
            () => {
              if (pendingArgs) {
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

/**
 * Converts a string into a valid JavaScript identifier.
 * This function replaces invalid characters with underscores,
 * ensures the name doesn't start with a digit, and appends
 * an underscore if the name is a reserved JavaScript keyword.
 *
 * @param name The input string to convert.
 * @returns A string that is a valid JavaScript identifier.
 */
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

/**
 * Extracts the service alias from a built-in tool name.
 * Parses the proxy-namespaced format: `<alias>__<toolname>`
 *
 * This format is used by `MCPServiceProxy` to route tool calls to the appropriate
 * session-isolated built-in server instance.
 *
 * IMPORTANT: Service aliases (names) can contain single underscores (e.g., `mcp_manager`, `attachments`)
 * but MUST NOT contain double underscores (`__`) as that is the delimiter between alias and tool name.
 *
 * Examples:
 * - `browser__clickElement` → `browser`
 * - `mcp_manager__list_servers` → `mcp_manager`
 * - `unknown_server__tool` → `null` (not a known builtin service)
 *
 * @param toolName The tool name to extract the alias from
 * @returns The service alias or null if the tool name doesn't match a known builtin service
 */
export function extractBuiltInServiceAlias(toolName: string): string | null {
  const idx = toolName.indexOf('__');
  // Require character before and after '__'
  if (idx <= 0 || idx + 2 >= toolName.length) return null;
  const server = toolName.slice(0, idx);
  return BUILTIN_SERVICE_NAMES.has(server) ? server : null;
}

/**
 * Validates a service alias (name) for use in built-in tool naming.
 *
 * Valid service names:
 * - Must not be empty
 * - Must not contain double underscores (`__`)
 * - Should use snake_case convention
 * - Can contain single underscores (e.g., `mcp_manager`, `attachments`)
 *
 * @param serviceAlias The service alias to validate
 * @returns true if valid, false otherwise
 */
export function isValidServiceAlias(serviceAlias: string): boolean {
  if (!serviceAlias || serviceAlias.trim().length === 0) {
    return false;
  }

  // Check for double underscores (reserved as delimiter)
  if (serviceAlias.includes('__')) {
    return false;
  }

  // Optional: Check for valid characters (alphanumeric and single underscore)
  // This allows names like: browser, mcp_manager, attachments, a_b_c_d
  const validPattern = /^[a-z0-9]+(_[a-z0-9]+)*$/i;
  return validPattern.test(serviceAlias);
}
