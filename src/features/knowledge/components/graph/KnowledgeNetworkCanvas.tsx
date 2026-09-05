import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  Flame,
  Loader2,
  Maximize2,
  Minus,
  Plus,
  RotateCcw,
} from 'lucide-react';
import { Button, Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui';
import type {
  KnowledgeGraphEntity,
  KnowledgeGraphRelationship,
} from '@/lib/backend/knowledge';
import { ForceSimulation } from './knowledge-graph-simulation';
import {
  type CameraTransform,
  getNodeColor,
  segmentIntersectsAabb,
  type SimulationLink,
  type SimulationNode,
} from './knowledge-graph-types';
import { KnowledgeNodePeekCard } from './KnowledgeNodePeekCard';

export interface KnowledgeNetworkCanvasProps {
  entities: KnowledgeGraphEntity[];
  relationships: KnowledgeGraphRelationship[];
  selectedEntityId?: number | null;
  onSelectEntity?: (entityId: number | null) => void;
  className?: string;
  emptyMessage?: string;
  isLoading?: boolean;
}

interface DragState {
  type: 'node' | 'pan';
  startX: number;
  startY: number;
  hasMoved: boolean;
  targetNode?: SimulationNode;
  initCameraX: number;
  initCameraY: number;
}

const labelWidthCache = new Map<string, number>();

export const KnowledgeNetworkCanvas = memo(function KnowledgeNetworkCanvas({
  entities,
  relationships,
  selectedEntityId = null,
  onSelectEntity,
  className = '',
  emptyMessage,
  isLoading = false,
}: KnowledgeNetworkCanvasProps) {
  const { t } = useTranslation('common');
  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  const [containerSize, setContainerSize] = useState({ width: 0, height: 0 });
  const [camera, setCamera] = useState<CameraTransform>({
    x: 400,
    y: 300,
    zoom: 1.0,
  });

  const [hoveredNode, setHoveredNode] = useState<SimulationNode | null>(null);
  const hoveredNodeRef = useRef<SimulationNode | null>(hoveredNode);
  hoveredNodeRef.current = hoveredNode;

  const [hoverScreenPos, setHoverScreenPos] = useState<{ x: number; y: number } | null>(null);
  const hasInitialFitRef = useRef(false);

  const lastInteractionTimeRef = useRef(Date.now());
  const isInteractingRef = useRef(false);

  const simulationRef = useRef<ForceSimulation>(new ForceSimulation());
  const cameraRef = useRef<CameraTransform>(camera);
  // Do not overwrite cameraRef from React state every render — applyCamera keeps
  // the ref in sync so rAF can see camera updates in the same frame as fit/pan/zoom.

  const containerSizeRef = useRef(containerSize);
  containerSizeRef.current = containerSize;

  const dragStateRef = useRef<DragState | null>(null);
  const animationFrameIdRef = useRef<number | null>(null);
  const renderRef = useRef<((time: number) => void) | null>(null);

  const requestRender = useCallback(() => {
    lastInteractionTimeRef.current = Date.now();
    if (!animationFrameIdRef.current && renderRef.current) {
      animationFrameIdRef.current = requestAnimationFrame(renderRef.current);
    }
  }, []);

  /** Sync cameraRef immediately so the same-frame rAF sees the new camera. */
  const applyCamera = useCallback(
    (next: CameraTransform) => {
      cameraRef.current = next;
      setCamera(next);
      requestRender();
    },
    [requestRender],
  );

  // 1. Convert entities and relationships into SimulationNode and SimulationLink
  const { nodes, links } = useMemo(() => {
    const degMap = new Map<number, number>();
    for (const rel of relationships) {
      degMap.set(rel.sourceEntityId, (degMap.get(rel.sourceEntityId) ?? 0) + 1);
      degMap.set(rel.targetEntityId, (degMap.get(rel.targetEntityId) ?? 0) + 1);
    }

    const simNodes: SimulationNode[] = entities.map((entity) => {
      const connCount = degMap.get(entity.id) ?? 0;
      const radius = entity.isPrimary
        ? 22
        : Math.min(20, Math.max(13, 13 + connCount * 1.5));

      return {
        id: entity.id,
        name: entity.name,
        entityType: entity.entityType,
        description: entity.description,
        isPrimary: entity.isPrimary,
        x: 0,
        y: 0,
        vx: 0,
        vy: 0,
        radius,
        color: getNodeColor(entity.entityType, entity.isPrimary, true),
        connectionCount: connCount,
        assistantId: entity.assistantId,
        rawEntity: entity,
      };
    });

    const nodeById = new Map<number, SimulationNode>();
    for (const n of simNodes) {
      nodeById.set(n.id, n);
    }

    const simLinks: SimulationLink[] = [];
    for (const rel of relationships) {
      const source = nodeById.get(rel.sourceEntityId);
      const target = nodeById.get(rel.targetEntityId);
      if (source && target) {
        simLinks.push({
          id: rel.id,
          source,
          target,
          sourceId: rel.sourceEntityId,
          targetId: rel.targetEntityId,
          relationType: rel.relationType,
          weight: rel.weight,
          rawRelationship: rel,
        });
      }
    }

    return { nodes: simNodes, links: simLinks, degreeMap: degMap };
  }, [entities, relationships]);

  // 2. Pre-calculate 1-hop focus neighborhood sets
  const { connectedEdgeIds, neighborNodeIds } = useMemo(() => {
    if (selectedEntityId === null || selectedEntityId === undefined) {
      return {
        connectedEdgeIds: new Set<string | number>(),
        neighborNodeIds: new Set<number>(),
      };
    }

    const edgeIds = new Set<string | number>();
    const neighborIds = new Set<number>([selectedEntityId]);

    for (const link of links) {
      if (link.sourceId === selectedEntityId || link.targetId === selectedEntityId) {
        edgeIds.add(link.id);
        neighborIds.add(link.sourceId);
        neighborIds.add(link.targetId);
      }
    }

    return {
      connectedEdgeIds: edgeIds,
      neighborNodeIds: neighborIds,
    };
  }, [selectedEntityId, links]);

  const prevEntitiesLengthRef = useRef(entities.length);

  // Reset initial fit flag and wake render loop when entities transition from 0 to >0
  useEffect(() => {
    if (prevEntitiesLengthRef.current === 0 && entities.length > 0) {
      lastInteractionTimeRef.current = Date.now();
      requestRender();
    }
    prevEntitiesLengthRef.current = entities.length;
    hasInitialFitRef.current = false;
  }, [entities, requestRender]);

  // 3. Keep simulation synchronized with data
  useEffect(() => {
    const sim = simulationRef.current;
    sim.setNodes(nodes, true);
    sim.setLinks(links);
    requestRender();
  }, [nodes, links, requestRender]);

  // Wake loop when selection changes externally
  useEffect(() => {
    requestRender();
  }, [selectedEntityId, requestRender]);

  // 4. Fit Camera to Nodes helper
  const fitCameraToNodes = useCallback(
    (nodesToFit: SimulationNode[], width: number, height: number) => {
      if (nodesToFit.length === 0 || width <= 0 || height <= 0) {
        applyCamera({
          x: width > 0 ? width / 2 : 400,
          y: height > 0 ? height / 2 : 300,
          zoom: 1.0,
        });
        return;
      }

      let minX = Infinity;
      let maxX = -Infinity;
      let minY = Infinity;
      let maxY = -Infinity;

      for (const node of nodesToFit) {
        const nodePadding = (node.radius || 20) + 30;
        if (node.x - nodePadding < minX) minX = node.x - nodePadding;
        if (node.x + nodePadding > maxX) maxX = node.x + nodePadding;
        if (node.y - nodePadding < minY) minY = node.y - nodePadding;
        if (node.y + nodePadding > maxY) maxY = node.y + nodePadding;
      }

      const graphWidth = Math.max(maxX - minX, 100);
      const graphHeight = Math.max(maxY - minY, 100);
      const centerX = (minX + maxX) / 2;
      const centerY = (minY + maxY) / 2;

      const scaleX = width / graphWidth;
      const scaleY = height / graphHeight;
      const targetZoom = Math.min(Math.max(Math.min(scaleX, scaleY), 0.25), 1.5);

      applyCamera({
        x: width / 2 - centerX * targetZoom,
        y: height / 2 - centerY * targetZoom,
        zoom: targetZoom,
      });
    },
    [applyCamera],
  );

  // 5. Auto-fit camera when nodes and container dimensions are first ready
  useEffect(() => {
    if (hasInitialFitRef.current) return;
    if (nodes.length > 0) {
      let width = containerSize.width;
      let height = containerSize.height;

      // If containerSize state hasn't updated yet, measure directly from DOM
      if (width <= 0 || height <= 0) {
        const rect = containerRef.current?.getBoundingClientRect();
        if (rect && rect.width > 0 && rect.height > 0) {
          width = rect.width;
          height = rect.height;
          setContainerSize({ width, height });
        }
      }

      if (width > 0 && height > 0) {
        const simNodes = simulationRef.current.nodes;
        const targetNodes = simNodes.length > 0 ? simNodes : nodes;
        fitCameraToNodes(targetNodes, width, height);
        hasInitialFitRef.current = true;
        requestRender();
      }
    }
  }, [nodes, containerSize, fitCameraToNodes, requestRender]);

  // 6. Resize observer for container and initial fit when container size becomes valid
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleResize = (width: number, height: number) => {
      if (width > 0 && height > 0) {
        setContainerSize((prev) => {
          if (prev.width === width && prev.height === height) {
            return prev;
          }
          return { width, height };
        });
        if (!hasInitialFitRef.current) {
          const simNodes = simulationRef.current.nodes;
          const targetNodes = simNodes.length > 0 ? simNodes : nodes;
          if (targetNodes.length > 0) {
            fitCameraToNodes(targetNodes, width, height);
            hasInitialFitRef.current = true;
          }
        } else {
          requestRender();
        }
      }
    };

    const rect = container.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      handleResize(rect.width, rect.height);
    }

    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver((entries) => {
        const entry = entries[0];
        if (!entry) return;
        const { width, height } = entry.contentRect;
        handleResize(width, height);
      });

      observer.observe(container);
      return () => observer.disconnect();
    }
  }, [fitCameraToNodes, requestRender, entities.length, nodes]);

  // 7. Center and Fit Camera
  const handleResetCamera = useCallback(() => {
    const simNodes = simulationRef.current.nodes;
    const targetNodes = simNodes.length > 0 ? simNodes : nodes;
    fitCameraToNodes(targetNodes, containerSize.width, containerSize.height);
  }, [containerSize, fitCameraToNodes, nodes]);

  // Re-heat simulation
  const handleReheat = useCallback(() => {
    simulationRef.current.reheat(1.0);
    requestRender();
  }, [requestRender]);

  // Zoom In / Out handlers
  const handleZoom = useCallback(
    (factor: number) => {
      const { width, height } = containerSizeRef.current;
      const centerScreenX = width / 2;
      const centerScreenY = height / 2;
      const prev = cameraRef.current;
      const worldX = (centerScreenX - prev.x) / prev.zoom;
      const worldY = (centerScreenY - prev.y) / prev.zoom;
      const newZoom = Math.min(Math.max(prev.zoom * factor, 0.2), 3.5);
      applyCamera({
        x: centerScreenX - worldX * newZoom,
        y: centerScreenY - worldY * newZoom,
        zoom: newZoom,
      });
    },
    [applyCamera],
  );

  // 6. High-performance Canvas Rendering Loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let isMounted = true;

    const render = (time: number) => {
      if (!isMounted) {
        animationFrameIdRef.current = null;
        return;
      }

      const sim = simulationRef.current;
      const isSimulating = sim.tick();

      const dpr = window.devicePixelRatio || 1;
      const { width, height } = containerSizeRef.current;

      if (width <= 0 || height <= 0) {
        // Layout not ready — keep polling until ResizeObserver supplies size.
        animationFrameIdRef.current = requestAnimationFrame(render);
        return;
      }

      if (canvas.width !== Math.floor(width * dpr) || canvas.height !== Math.floor(height * dpr)) {
        canvas.width = Math.floor(width * dpr);
        canvas.height = Math.floor(height * dpr);
        canvas.style.width = `${width}px`;
        canvas.style.height = `${height}px`;
      }

      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, width, height);

      const isDark = document.documentElement.classList.contains('dark');
      const cam = cameraRef.current;
      const zoom = cam.zoom;

      // Viewport bounds in world coordinates
      const minWorldX = -cam.x / zoom;
      const minWorldY = -cam.y / zoom;
      const maxWorldX = (width - cam.x) / zoom;
      const maxWorldY = (height - cam.y) / zoom;
      const margin = 60; // safety margin for node radius & labels

      // Draw World coordinate transforms
      ctx.save();
      ctx.translate(cam.x, cam.y);
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

      const hasSelection = selectedEntityId !== null && selectedEntityId !== undefined;
      const simLinks = sim.links;
      const simNodes = sim.nodes;

      // B. Render Edges
      for (let i = 0; i < simLinks.length; i++) {
        const link = simLinks[i];
        const s = link.source;
        const t = link.target;

        // Skip only when the segment does not intersect the viewport AABB
        // (both endpoints outside can still cross the visible rect).
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

        // Subtle curve offset
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

        // Edge label text at curve apex
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

      for (let i = 0; i < simNodes.length; i++) {
        const node = simNodes[i];

        // Viewport frustum culling: skip if outside visible rect
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
        const isHovered = hoveredNodeRef.current?.id === node.id;

        ctx.save();
        if (isDimmed) {
          ctx.globalAlpha = 0.15;
        }

        const nodeColor = getNodeColor(node.entityType, node.isPrimary, isDark);

        // 1. Primary node glowing pulsating ring
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

        // 2. Selection / Focus Ring
        if (isSelected || isHovered) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius + 5, 0, Math.PI * 2);
          ctx.strokeStyle = isDark ? '#38bdf8' : '#0284c7';
          ctx.lineWidth = isSelected ? 3 : 2;
          ctx.stroke();
        }

        // 3. Main Node Body
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
          // Secondary node badge
          ctx.fillStyle = isDark ? '#1e293b' : '#ffffff';
          ctx.fill();

          ctx.strokeStyle = nodeColor;
          ctx.lineWidth = 2.5;
          ctx.stroke();

          // Small inner color accent dot
          ctx.beginPath();
          ctx.arc(node.x, node.y, 4, 0, Math.PI * 2);
          ctx.fillStyle = nodeColor;
          ctx.fill();
        }

        // 4. Node Label Text
        const textY = node.y + node.radius + 13;
        ctx.font = node.isPrimary
          ? '600 11px system-ui, -apple-system, sans-serif'
          : '500 10px system-ui, -apple-system, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';

        const displayName =
          node.name.length > 18 ? `${node.name.slice(0, 16)}…` : node.name;

        // Outline background for readability
        ctx.strokeStyle = isDark ? '#09090b' : '#ffffff';
        ctx.lineWidth = 3;
        ctx.strokeText(displayName, node.x, textY);

        ctx.fillStyle = isDark ? '#f8fafc' : '#0f172a';
        ctx.fillText(displayName, node.x, textY);

        ctx.restore();
      }

      ctx.restore(); // restore world transform
      ctx.restore(); // restore dpr scale

      // Idle sleep: never sleep before the first camera fit, otherwise the canvas
      // stays blank while hit-testing already uses the updated camera after re-render.
      const isInteracting =
        isInteractingRef.current ||
        Date.now() - lastInteractionTimeRef.current < 800;
      const needsPulse =
        entities.some((e) => e.isPrimary) &&
        Date.now() - lastInteractionTimeRef.current < 3000;
      const awaitingInitialFit =
        entities.length > 0 && !hasInitialFitRef.current;

      if (
        !isSimulating &&
        !isInteracting &&
        !needsPulse &&
        !awaitingInitialFit
      ) {
        animationFrameIdRef.current = null;
        return;
      }

      animationFrameIdRef.current = requestAnimationFrame(render);
    };

    renderRef.current = render;
    animationFrameIdRef.current = requestAnimationFrame(render);

    return () => {
      isMounted = false;
      renderRef.current = null;
      if (animationFrameIdRef.current !== null) {
        cancelAnimationFrame(animationFrameIdRef.current);
        animationFrameIdRef.current = null;
      }
    };
  }, [
    containerSize,
    selectedEntityId,
    connectedEdgeIds,
    neighborNodeIds,
    entities,
  ]);

  // 7. Mouse / Pointer Interaction Handlers
  const handlePointerDown = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      isInteractingRef.current = true;
      requestRender();

      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;

      const cam = cameraRef.current;
      const worldX = (screenX - cam.x) / cam.zoom;
      const worldY = (screenY - cam.y) / cam.zoom;

      const hitNode = simulationRef.current.getNodeAt(worldX, worldY);

      if (hitNode) {
        hitNode.pinned = true;
        dragStateRef.current = {
          type: 'node',
          startX: e.clientX,
          startY: e.clientY,
          hasMoved: false,
          targetNode: hitNode,
          initCameraX: cam.x,
          initCameraY: cam.y,
        };
        simulationRef.current.reheat(0.4);
      } else {
        dragStateRef.current = {
          type: 'pan',
          startX: e.clientX,
          startY: e.clientY,
          hasMoved: false,
          initCameraX: cam.x,
          initCameraY: cam.y,
        };
      }

      canvas.setPointerCapture(e.pointerId);
    },
    [requestRender],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      requestRender();

      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;

      const cam = cameraRef.current;
      const worldX = (screenX - cam.x) / cam.zoom;
      const worldY = (screenY - cam.y) / cam.zoom;

      const drag = dragStateRef.current;

      if (drag) {
        const deltaDist = Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY);
        if (deltaDist > 3) {
          drag.hasMoved = true;
        }

        if (drag.type === 'node' && drag.targetNode) {
          drag.targetNode.x = worldX;
          drag.targetNode.y = worldY;
          drag.targetNode.vx = 0;
          drag.targetNode.vy = 0;
          simulationRef.current.reheat(0.25);
          hoveredNodeRef.current = null;
          setHoveredNode(null);
          setHoverScreenPos(null);
          return;
        }

        if (drag.type === 'pan') {
          const dx = e.clientX - drag.startX;
          const dy = e.clientY - drag.startY;
          applyCamera({
            x: drag.initCameraX + dx,
            y: drag.initCameraY + dy,
            zoom: cam.zoom,
          });
          return;
        }
      }

      // Normal hover test
      const hit = simulationRef.current.getNodeAt(worldX, worldY);
      if (hit !== hoveredNodeRef.current) {
        hoveredNodeRef.current = hit;
        setHoveredNode(hit);
        requestRender();
        if (hit) {
          const nodeScreenX = hit.x * cam.zoom + cam.x;
          const nodeScreenY = hit.y * cam.zoom + cam.y;
          setHoverScreenPos({ x: nodeScreenX, y: nodeScreenY });
        } else {
          setHoverScreenPos(null);
        }
      } else if (hit) {
        const nodeScreenX = hit.x * cam.zoom + cam.x;
        const nodeScreenY = hit.y * cam.zoom + cam.y;
        setHoverScreenPos({ x: nodeScreenX, y: nodeScreenY });
      }
    },
    [applyCamera, requestRender],
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (canvas && canvas.hasPointerCapture(e.pointerId)) {
        canvas.releasePointerCapture(e.pointerId);
      }

      isInteractingRef.current = false;
      requestRender();

      const drag = dragStateRef.current;
      if (!drag) return;

      if (drag.type === 'node' && drag.targetNode) {
        drag.targetNode.pinned = false;
        simulationRef.current.reheat(0.3);

        // Click selection if not moved
        if (!drag.hasMoved) {
          const nextSelection =
            selectedEntityId === drag.targetNode.id ? null : drag.targetNode.id;
          onSelectEntity?.(nextSelection);
        }
      } else if (drag.type === 'pan') {
        // Background click clears selection
        if (!drag.hasMoved) {
          onSelectEntity?.(null);
        }
      }

      dragStateRef.current = null;
    },
    [selectedEntityId, onSelectEntity, requestRender],
  );

  const handlePointerLeave = useCallback(() => {
    if (hoveredNodeRef.current) {
      hoveredNodeRef.current = null;
      setHoveredNode(null);
      setHoverScreenPos(null);
      requestRender();
    }
  }, [requestRender]);

  // Wheel Zoom focused on cursor
  const handleWheel = useCallback(
    (e: React.WheelEvent<HTMLCanvasElement>) => {
      e.preventDefault();

      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;
      const prev = cameraRef.current;
      const worldX = (mouseX - prev.x) / prev.zoom;
      const worldY = (mouseY - prev.y) / prev.zoom;
      const zoomFactor = e.deltaY < 0 ? 1.15 : 0.87;
      const newZoom = Math.min(Math.max(prev.zoom * zoomFactor, 0.2), 3.5);

      applyCamera({
        x: mouseX - worldX * newZoom,
        y: mouseY - worldY * newZoom,
        zoom: newZoom,
      });
    },
    [applyCamera],
  );

  return (
    <div
      ref={containerRef}
      className={`relative h-full w-full select-none overflow-hidden rounded-2xl border border-border/60 bg-background/50 backdrop-blur-sm ${className}`}
    >
      <canvas
        ref={canvasRef}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
        onPointerLeave={handlePointerLeave}
        onWheel={handleWheel}
        className={`block h-full w-full touch-none ${
          entities.length === 0
            ? 'cursor-default pointer-events-none'
            : 'cursor-grab active:cursor-grabbing'
        }`}
      />

      {/* Empty / Loading State Overlay */}
      {entities.length === 0 && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center p-8">
          {isLoading ? (
            <div className="flex flex-col items-center gap-2 text-muted-foreground">
              <Loader2 className="h-6 w-6 animate-spin text-primary" />
              <span className="text-xs font-medium">
                {t('knowledge.graph.loading', 'Loading knowledge graph...')}
              </span>
            </div>
          ) : (
            <div className="rounded-2xl border border-dashed border-border/60 bg-muted/10 p-8 text-center text-sm text-muted-foreground">
              {emptyMessage ??
                t(
                  'knowledge.graph.empty',
                  'No knowledge entities found in the graph.',
                )}
            </div>
          )}
        </div>
      )}

      {/* Floating Glassmorphism Node Peek Card */}
      {entities.length > 0 && (
        <KnowledgeNodePeekCard
          node={hoveredNode}
          position={hoverScreenPos}
          containerBounds={containerSize}
        />
      )}

      {/* Controls Overlay */}
      {entities.length > 0 && (
        <div className="absolute bottom-4 right-4 flex items-center gap-1.5 rounded-xl border border-border/60 bg-background/80 p-1 shadow-lg backdrop-blur-md">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-muted-foreground hover:text-foreground"
                  onClick={() => handleZoom(1.2)}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="top">
                {t('knowledge.graph.zoomIn', 'Zoom In')}
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-muted-foreground hover:text-foreground"
                  onClick={() => handleZoom(0.83)}
                >
                  <Minus className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="top">
                {t('knowledge.graph.zoomOut', 'Zoom Out')}
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-muted-foreground hover:text-foreground"
                  onClick={handleResetCamera}
                >
                  <Maximize2 className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="top">
                {t('knowledge.graph.resetView', 'Reset View')}
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-muted-foreground hover:text-foreground"
                  onClick={handleReheat}
                >
                  <Flame className="h-4 w-4 text-amber-500 dark:text-amber-400" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="top">
                {t('knowledge.graph.reheatPhysics', 'Reheat Physics')}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      )}

      {/* Top Left Stats & Focus Indicator */}
      {entities.length > 0 && (
        <div className="pointer-events-none absolute left-4 top-4 flex flex-col gap-1.5 text-xs">
          <div className="flex items-center gap-2 rounded-lg border border-border/50 bg-background/70 px-2.5 py-1 text-muted-foreground shadow-sm backdrop-blur-md">
            <span>
              {entities.length}{' '}
              {t('knowledge.graph.entitiesCount', 'entities')}
            </span>
            <span className="opacity-40">•</span>
            <span>
              {relationships.length}{' '}
              {t('knowledge.graph.relationsCount', 'relations')}
            </span>
          </div>

          {selectedEntityId !== null && (
            <div className="flex items-center gap-1.5 rounded-lg border border-primary/30 bg-primary/10 px-2.5 py-1 text-primary shadow-sm backdrop-blur-md">
              <span className="font-medium">
                {t('knowledge.graph.focusedMode', '1-Hop Focused')}
              </span>
              <button
                type="button"
                className="pointer-events-auto ml-1 text-primary/70 hover:text-primary"
                onClick={() => {
                  onSelectEntity?.(null);
                  requestRender();
                }}
              >
                <RotateCcw className="h-3 w-3" />
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
});
