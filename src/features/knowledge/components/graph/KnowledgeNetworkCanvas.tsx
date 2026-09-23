import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Flame,
  Loader2,
  Maximize2,
  Minus,
  Plus,
  RotateCcw,
} from 'lucide-react';
import {
  Button,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui';
import type {
  KnowledgeGraphEntity,
  KnowledgeGraphRelationship,
} from '@/lib/backend/knowledge';
import { ForceSimulation } from './knowledge-graph-simulation';
import {
  getNodeColor,
  type SimulationLink,
  type SimulationNode,
} from './knowledge-graph-types';
import { KnowledgeNodePeekCard } from './KnowledgeNodePeekCard';
import { renderGraphCanvas } from './renderGraphCanvas';
import { useCanvasCamera } from './useCanvasCamera';
import { useCanvasInteraction } from './useCanvasInteraction';

export interface KnowledgeNetworkCanvasProps {
  entities: KnowledgeGraphEntity[];
  relationships: KnowledgeGraphRelationship[];
  selectedEntityId?: number | null;
  onSelectEntity?: (entityId: number | null) => void;
  className?: string;
  emptyMessage?: string;
  isLoading?: boolean;
}

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
  const containerSizeRef = useRef(containerSize);
  containerSizeRef.current = containerSize;

  const lastInteractionTimeRef = useRef(Date.now());
  const isInteractingRef = useRef(false);
  const simulationRef = useRef<ForceSimulation>(new ForceSimulation());
  const animationFrameIdRef = useRef<number | null>(null);
  const renderRef = useRef<((time: number) => void) | null>(null);

  const requestRender = useCallback(() => {
    lastInteractionTimeRef.current = Date.now();
    if (!animationFrameIdRef.current && renderRef.current) {
      animationFrameIdRef.current = requestAnimationFrame(renderRef.current);
    }
  }, []);

  const markInteracting = useCallback((active: boolean) => {
    isInteractingRef.current = active;
  }, []);

  const {
    cameraRef,
    applyCamera,
    fitCameraToNodes,
    hasInitialFitRef,
    zoomByFactor,
    zoomAtScreenPoint,
  } = useCanvasCamera(requestRender);

  const {
    hoveredNode,
    hoveredNodeRef,
    hoverScreenPos,
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
    handlePointerLeave,
    handleWheel,
  } = useCanvasInteraction({
    canvasRef,
    simulationRef,
    cameraRef,
    applyCamera,
    requestRender,
    markInteracting,
    zoomAtScreenPoint,
    selectedEntityId,
    onSelectEntity,
  });

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

    return { nodes: simNodes, links: simLinks };
  }, [entities, relationships]);

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
      if (
        link.sourceId === selectedEntityId ||
        link.targetId === selectedEntityId
      ) {
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

  useEffect(() => {
    if (prevEntitiesLengthRef.current === 0 && entities.length > 0) {
      lastInteractionTimeRef.current = Date.now();
      requestRender();
    }
    prevEntitiesLengthRef.current = entities.length;
    hasInitialFitRef.current = false;
  }, [entities, hasInitialFitRef, requestRender]);

  useEffect(() => {
    const sim = simulationRef.current;
    sim.setNodes(nodes, true);
    sim.setLinks(links);
    requestRender();
  }, [nodes, links, requestRender]);

  useEffect(() => {
    requestRender();
  }, [selectedEntityId, requestRender]);

  useEffect(() => {
    if (hasInitialFitRef.current) return;
    if (nodes.length > 0) {
      let width = containerSize.width;
      let height = containerSize.height;

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
  }, [nodes, containerSize, fitCameraToNodes, hasInitialFitRef, requestRender]);

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
  }, [
    fitCameraToNodes,
    hasInitialFitRef,
    requestRender,
    entities.length,
    nodes,
  ]);

  const handleResetCamera = useCallback(() => {
    const simNodes = simulationRef.current.nodes;
    const targetNodes = simNodes.length > 0 ? simNodes : nodes;
    fitCameraToNodes(targetNodes, containerSize.width, containerSize.height);
  }, [containerSize, fitCameraToNodes, nodes]);

  const handleReheat = useCallback(() => {
    simulationRef.current.reheat(1.0);
    requestRender();
  }, [requestRender]);

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
        animationFrameIdRef.current = requestAnimationFrame(render);
        return;
      }

      if (
        canvas.width !== Math.floor(width * dpr) ||
        canvas.height !== Math.floor(height * dpr)
      ) {
        canvas.width = Math.floor(width * dpr);
        canvas.height = Math.floor(height * dpr);
        canvas.style.width = `${width}px`;
        canvas.style.height = `${height}px`;
      }

      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, width, height);

      const isDark = document.documentElement.classList.contains('dark');

      renderGraphCanvas({
        ctx,
        width,
        height,
        camera: cameraRef.current,
        nodes: sim.nodes,
        links: sim.links,
        selectedEntityId,
        connectedEdgeIds,
        neighborNodeIds,
        hoveredNodeId: hoveredNodeRef.current?.id ?? null,
        time,
        isDark,
      });

      ctx.restore();

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
    cameraRef,
    connectedEdgeIds,
    entities,
    hasInitialFitRef,
    hoveredNodeRef,
    neighborNodeIds,
    selectedEntityId,
  ]);

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

      {entities.length > 0 && (
        <KnowledgeNodePeekCard
          node={hoveredNode}
          position={hoverScreenPos}
          containerBounds={containerSize}
        />
      )}

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
                  onClick={() => zoomByFactor(1.2, containerSize)}
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
                  onClick={() => zoomByFactor(0.83, containerSize)}
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

      {entities.length > 0 && (
        <div className="pointer-events-none absolute left-4 top-4 flex flex-col gap-1.5 text-xs">
          <div className="flex items-center gap-2 rounded-lg border border-border/50 bg-background/70 px-2.5 py-1 text-muted-foreground shadow-sm backdrop-blur-md">
            <span>
              {entities.length} {t('knowledge.graph.entitiesCount', 'entities')}
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
