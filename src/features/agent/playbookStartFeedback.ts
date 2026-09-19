/** Stable sonner toast id bridging StartView → ChatView across navigation. */
export function playbookStartToastId(playbookId: string): string {
  return `playbook-start:${playbookId}`;
}
