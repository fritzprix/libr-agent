/**
 * Unread / attention indicator on panel toggle buttons.
 * Same convention as notification badges: a small primary dot when there
 * are updates while the panel is closed (no auto-open).
 */
interface PanelAttentionDotProps {
  visible: boolean;
}

export function PanelAttentionDot({ visible }: PanelAttentionDotProps) {
  if (!visible) {
    return null;
  }

  return (
    <span
      data-testid="panel-attention-dot"
      className="pointer-events-none absolute top-0.5 right-0.5 h-1.5 w-1.5 rounded-full bg-primary"
      aria-hidden
    />
  );
}
