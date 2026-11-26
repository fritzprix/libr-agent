// Console-only logger shim for the screenshot harness (the real one calls Tauri invoke).
const noop = () => {};

class ConsoleLogger {
  debug = (...a: unknown[]) => console.debug(...a);
  info = (...a: unknown[]) => console.info(...a);
  warn = (...a: unknown[]) => console.warn(...a);
  error = (...a: unknown[]) => console.error(...a);
  trace = noop;
  setLevel = noop;
}

const instance = new ConsoleLogger();

export function getLogger(): ConsoleLogger {
  return instance;
}
export const log = instance;
