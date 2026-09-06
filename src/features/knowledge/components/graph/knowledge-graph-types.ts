import type {
  KnowledgeGraphEntity,
  KnowledgeGraphRelationship,
} from '@/lib/backend/knowledge';

export interface SimulationNode {
  id: number;
  name: string;
  entityType?: string | null;
  description?: string | null;
  isPrimary: boolean;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  pinned?: boolean;
  connectionCount: number;
  assistantId?: string;
  rawEntity?: KnowledgeGraphEntity;
}

export interface SimulationLink {
  id: number | string;
  source: SimulationNode;
  target: SimulationNode;
  sourceId: number;
  targetId: number;
  relationType: string;
  weight: number;
  rawRelationship?: KnowledgeGraphRelationship;
}

export interface GraphFocusState {
  focusedNodeId: number | null;
  hoveredNodeId: number | null;
  selectedNodeId?: number | null;
}

export interface CameraTransform {
  x: number;
  y: number;
  zoom: number;
}

export interface CanvasPoint {
  x: number;
  y: number;
}

export interface KnowledgeGraphVisualTheme {
  isDark: boolean;
  background: string;
  gridDot: string;
  nodePrimary: string;
  nodePrimaryGlow: string;
  nodeSecondary: string;
  nodeSecondaryBorder: string;
  nodeSelectedRing: string;
  nodeText: string;
  nodeTextStroke: string;
  edgeStroke: string;
  edgeStrokeHighlighted: string;
  edgeLabelBg: string;
  edgeLabelBorder: string;
  edgeLabelText: string;
  dimmedAlpha: number;
}

export const ENTITY_TYPE_COLORS: Record<
  string,
  { light: string; dark: string; borderLight: string; borderDark: string }
> = {
  concept: {
    light: '#10b981',
    dark: '#34d399',
    borderLight: '#059669',
    borderDark: '#059669',
  },
  technology: {
    light: '#0ea5e9',
    dark: '#38bdf8',
    borderLight: '#0284c7',
    borderDark: '#0284c7',
  },
  tool: {
    light: '#06b6d4',
    dark: '#22d3ee',
    borderLight: '#0891b2',
    borderDark: '#0891b2',
  },
  person: {
    light: '#f59e0b',
    dark: '#fbbf24',
    borderLight: '#d97706',
    borderDark: '#d97706',
  },
  organization: {
    light: '#8b5cf6',
    dark: '#a78bfa',
    borderLight: '#7c3aed',
    borderDark: '#7c3aed',
  },
  project: {
    light: '#6366f1',
    dark: '#818cf8',
    borderLight: '#4f46e5',
    borderDark: '#4f46e5',
  },
  rule: {
    light: '#ef4444',
    dark: '#f87171',
    borderLight: '#dc2626',
    borderDark: '#dc2626',
  },
  convention: {
    light: '#ec4899',
    dark: '#f472b6',
    borderLight: '#db2777',
    borderDark: '#db2777',
  },
};

export function getNodeColor(
  entityType: string | null | undefined,
  isPrimary: boolean,
  isDark: boolean,
): string {
  if (isPrimary) {
    return isDark ? '#818cf8' : '#6366f1';
  }

  const normalized = entityType?.toLowerCase().trim();
  if (normalized && ENTITY_TYPE_COLORS[normalized]) {
    return isDark
      ? ENTITY_TYPE_COLORS[normalized].dark
      : ENTITY_TYPE_COLORS[normalized].light;
  }

  return isDark ? '#94a3b8' : '#64748b';
}

/**
 * Liang–Barsky: true if segment (x0,y0)–(x1,y1) intersects the inclusive AABB.
 * Catches edges that cross the viewport even when both endpoints are outside.
 */
export function segmentIntersectsAabb(
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  minX: number,
  minY: number,
  maxX: number,
  maxY: number,
): boolean {
  let t0 = 0;
  let t1 = 1;
  const dx = x1 - x0;
  const dy = y1 - y0;

  const clip = (p: number, q: number): boolean => {
    if (p === 0) {
      return q >= 0;
    }
    const r = q / p;
    if (p < 0) {
      if (r > t1) return false;
      if (r > t0) t0 = r;
    } else {
      if (r < t0) return false;
      if (r < t1) t1 = r;
    }
    return true;
  };

  return (
    clip(-dx, x0 - minX) &&
    clip(dx, maxX - x0) &&
    clip(-dy, y0 - minY) &&
    clip(dy, maxY - y0)
  );
}
