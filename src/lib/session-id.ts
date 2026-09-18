/** Canonical session id: 10 lowercase hex chars (matches Rust `generate_session_id`). */
export function generateSessionId(): string {
  return crypto.randomUUID().replace(/-/g, '').slice(0, 10);
}
