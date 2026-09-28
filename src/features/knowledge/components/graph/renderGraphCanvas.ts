import {
  type CameraTransform,
  getNodeColor,
  segmentIntersectsAabb,
  type SimulationLink,
  type SimulationNode,
} from './knowledge-graph-types';

const labelWidthCache = new Map<string, number>();

export interface RenderGraphCanvasParams {
  ctx: CanvasRenderingContext2D;
  width: number;
  height: number;
  camera: CameraTransform;
  nodes: SimulationNode[];
  links: SimulationLink[];
  selectedEntityId: number | null | undefined;
  connectedEdgeIds: Set<string | number>;
  neighborNodeIds: Set<number>;
  hoveredNodeId: number | null;
  time: number;
  isDark: boolean;
}

/**
 * Draw the knowledge graph in world space (caller applies DPR + clears canvas).
 */
export function renderGraphCanvas({
  ctx,
  width,
  height,
  camera,
  nodes,
  links,
  selectedEntityId,
  connectedEdgeIds,
  neighborNodeIds,
  hoveredNodeId,
  time,
  isDark,
}: RenderGraphCanvasParams): void {
  const zoom = camera.zoom;

  const minWorldX = -camera.x / zoom;
  const minWorldY = -camera.y / zoom;
  const maxWorldX = (width - camera.x) / zoom;
  const maxWorldY = (height - camera.y) / zoom;
  const margin = 60;

  ctx.save();
  ctx.translate(camera.x, camera.y);
  ctx.scale(zoom, zoom);

  // A. Subtle background grid dots in world coordinates
  const gridSize = 40;
  const gridStep = Math.max(gridSize, Math.floor(20 / zoom) * 20);
  const startX = Math.floor(minWorldX / gridStep) * gridStep - gridStep;
  const endX = Math.ceil(maxWorldX / gridStep) * gridStep + gridStep;
  const startY = Math.floor(minWorldY / gridStep) * gridStep - gridStep;
  const endY = Math.ceil(maxWorldY / gridStep) * gridStep + gridStep;

  ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.08)' : 'rgba(0, 0, 0, 0.06)';
  for (let gx = startX; gx <= endX; gx += gridStep) {
    for (let gy = startY; gy <= endY; gy += gridStep) {
      ctx.beginPath();
      ctx.arc(gx, gy, 1, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  const hasSelection =
    selectedEntityId !== null && selectedEntityId !== undefined;

  // B. Render Edges
  for (let i = 0; i < links.length; i++) {
    const link = links[i];
    const s = link.source;
    const t = link.target;

    if (
      !segmentIntersectsAabb(
        s.x,
        s.y,
        t.x,
        t.y,
        minWorldX - margin,
        minWorldY - margin,
        maxWorldX + margin,
        maxWorldY + margin,
      )
    ) {
      continue;
    }

    const isHighlighted = hasSelection && connectedEdgeIds.has(link.id);
    const isDimmed = hasSelection && !isHighlighted;

    ctx.save();
    if (isDimmed) {
      ctx.globalAlpha = 0.12;
    }

    const midX = (s.x + t.x) / 2;
    const midY = (s.y + t.y) / 2;
    const dx = t.x - s.x;
    const dy = t.y - s.y;
    const len = Math.hypot(dx, dy) || 1;
    const nx = -dy / len;
    const ny = dx / len;

    const curveOffset = 14;
    const cpX = midX + nx * curveOffset;
    const cpY = midY + ny * curveOffset;

    ctx.beginPath();
    ctx.moveTo(s.x, s.y);
    ctx.quadraticCurveTo(cpX, cpY, t.x, t.y);

    if (isHighlighted) {
      ctx.strokeStyle = isDark ? '#38bdf8' : '#0284c7';
      ctx.lineWidth = 2.5;
    } else {
      ctx.strokeStyle = isDark
        ? 'rgba(148, 163, 184, 0.28)'
        : 'rgba(100, 116, 139, 0.24)';
      ctx.lineWidth = 1.5;
    }
    ctx.stroke();

    if ((zoom >= 0.7 || isHighlighted) && link.relationType) {
      const apexX = 0.25 * s.x + 0.5 * cpX + 0.25 * t.x;
      const apexY = 0.25 * s.y + 0.5 * cpY + 0.25 * t.y;

      ctx.font = '9px system-ui, -apple-system, sans-serif';
      const labelText = link.relationType;
      let textWidth = labelWidthCache.get(labelText);
      if (textWidth === undefined) {
        textWidth = ctx.measureText(labelText).width;
        labelWidthCache.set(labelText, textWidth);
      }
      const badgeW = textWidth + 10;
      const badgeH = 15;

      ctx.fillStyle = isDark ? '#18181b' : '#ffffff';
      ctx.strokeStyle = isHighlighted
        ? isDark
          ? '#38bdf8'
          : '#0284c7'
        : isDark
          ? '#3f3f46'
          : '#e2e8f0';
      ctx.lineWidth = 1;

      ctx.beginPath();
      ctx.roundRect(apexX - badgeW / 2, apexY - badgeH / 2, badgeW, badgeH, 4);
      ctx.fill();
      ctx.stroke();

      ctx.fillStyle = isHighlighted
        ? isDark
          ? '#38bdf8'
          : '#0284c7'
        : isDark
          ? '#a1a1aa'
          : '#64748b';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(labelText, apexX, apexY);
    }

    ctx.restore();
  }

  // C. Render Nodes
  const pulseTime = time * 0.003;

  for (let i = 0; i < nodes.length; i++) {
    const node = nodes[i];

    if (
      node.x < minWorldX - margin ||
      node.x > maxWorldX + margin ||
      node.y < minWorldY - margin ||
      node.y > maxWorldY + margin
    ) {
      continue;
    }

    const isSelected = hasSelection && selectedEntityId === node.id;
    const isNeighbor = hasSelection && neighborNodeIds.has(node.id);
    const isDimmed = hasSelection && !isNeighbor;
    const isHovered = hoveredNodeId === node.id;

    ctx.save();
    if (isDimmed) {
      ctx.globalAlpha = 0.15;
    }

    const nodeColor = getNodeColor(node.entityType, node.isPrimary, isDark);

    if (node.isPrimary) {
      const pulse = Math.sin(pulseTime) * 3 + 5;
      const glowRadius = node.radius + pulse;

      const glowGrad = ctx.createRadialGradient(
        node.x,
        node.y,
        node.radius,
        node.x,
        node.y,
        glowRadius + 6,
      );
      glowGrad.addColorStop(
        0,
        isDark ? 'rgba(129, 140, 248, 0.45)' : 'rgba(99, 102, 241, 0.35)',
      );
      glowGrad.addColorStop(1, 'rgba(99, 102, 241, 0)');

      ctx.beginPath();
      ctx.arc(node.x, node.y, glowRadius + 6, 0, Math.PI * 2);
      ctx.fillStyle = glowGrad;
      ctx.fill();
    }

    if (isSelected || isHovered) {
      ctx.beginPath();
      ctx.arc(node.x, node.y, node.radius + 5, 0, Math.PI * 2);
      ctx.strokeStyle = isDark ? '#38bdf8' : '#0284c7';
      ctx.lineWidth = isSelected ? 3 : 2;
      ctx.stroke();
    }

    ctx.beginPath();
    ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);

    if (node.isPrimary) {
      const fillGrad = ctx.createRadialGradient(
        node.x - node.radius * 0.3,
        node.y - node.radius * 0.3,
        2,
        node.x,
        node.y,
        node.radius,
      );
      if (isDark) {
        fillGrad.addColorStop(0, '#a5b4fc');
        fillGrad.addColorStop(1, '#6366f1');
      } else {
        fillGrad.addColorStop(0, '#818cf8');
        fillGrad.addColorStop(1, '#4f46e5');
      }
      ctx.fillStyle = fillGrad;
      ctx.fill();

      ctx.strokeStyle = isDark ? '#c7d2fe' : '#ffffff';
      ctx.lineWidth = 2;
      ctx.stroke();
    } else {
      ctx.fillStyle = isDark ? '#1e293b' : '#ffffff';
      ctx.fill();

      ctx.strokeStyle = nodeColor;
      ctx.lineWidth = 2.5;
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(node.x, node.y, 4, 0, Math.PI * 2);
      ctx.fillStyle = nodeColor;
      ctx.fill();
    }

    const textY = node.y + node.radius + 13;
    ctx.font = node.isPrimary
      ? '600 11px system-ui, -apple-system, sans-serif'
      : '500 10px system-ui, -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    const displayName =
      node.name.length > 18 ? `${node.name.slice(0, 16)}…` : node.name;

    ctx.strokeStyle = isDark ? '#09090b' : '#ffffff';
    ctx.lineWidth = 3;
    ctx.strokeText(displayName, node.x, textY);

    ctx.fillStyle = isDark ? '#f8fafc' : '#0f172a';
    ctx.fillText(displayName, node.x, textY);

    ctx.restore();
  }

  ctx.restore();
}
