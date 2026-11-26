export function safeInvoke(_command: string, _args?: unknown): Promise<never> {
  return Promise.resolve(undefined as never);
}
