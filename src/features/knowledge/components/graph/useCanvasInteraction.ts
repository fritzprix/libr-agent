import {
  useCallback,
  useRef,
  useState,
  type MutableRefObject,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  type WheelEvent as ReactWheelEvent,
} from 'react';
import type { ForceSimulation } from './knowledge-graph-simulation';
import type { CameraTransform, SimulationNode } from './knowledge-graph-types';

interface DragState {
  type: 'node' | 'pan';
  startX: number;
  startY: number;
  hasMoved: boolean;
  targetNode?: SimulationNode;
  initCameraX: number;
  initCameraY: number;
}

export interface UseCanvasInteractionParams {
  canvasRef: RefObject<HTMLCanvasElement | null>;
  simulationRef: MutableRefObject<ForceSimulation>;
  cameraRef: MutableRefObject<CameraTransform>;
  applyCamera: (next: CameraTransform) => void;
  requestRender: () => void;
  markInteracting: (active: boolean) => void;
  zoomAtScreenPoint: (screenX: number, screenY: number, factor: number) => void;
  selectedEntityId?: number | null;
  onSelectEntity?: (entityId: number | null) => void;
}

export function useCanvasInteraction({
  canvasRef,
  simulationRef,
  cameraRef,
  applyCamera,
  requestRender,
  markInteracting,
  zoomAtScreenPoint,
  selectedEntityId = null,
  onSelectEntity,
}: UseCanvasInteractionParams) {
  const [hoveredNode, setHoveredNode] = useState<SimulationNode | null>(null);
  const hoveredNodeRef = useRef<SimulationNode | null>(hoveredNode);
  hoveredNodeRef.current = hoveredNode;

  const [hoverScreenPos, setHoverScreenPos] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const dragStateRef = useRef<DragState | null>(null);

  const handlePointerDown = useCallback(
    (e: ReactPointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      const simulation = simulationRef.current;
      const cam = cameraRef.current;
      if (!canvas || !simulation || !cam) return;

      markInteracting(true);
      requestRender();

      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;

      const worldX = (screenX - cam.x) / cam.zoom;
      const worldY = (screenY - cam.y) / cam.zoom;

      const hitNode = simulation.getNodeAt(worldX, worldY);

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
        simulation.reheat(0.4);
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
    [cameraRef, canvasRef, markInteracting, requestRender, simulationRef],
  );

  const handlePointerMove = useCallback(
    (e: ReactPointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      const simulation = simulationRef.current;
      const cam = cameraRef.current;
      if (!canvas || !simulation || !cam) return;

      requestRender();

      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;

      const worldX = (screenX - cam.x) / cam.zoom;
      const worldY = (screenY - cam.y) / cam.zoom;

      const drag = dragStateRef.current;

      if (drag) {
        const deltaDist = Math.hypot(
          e.clientX - drag.startX,
          e.clientY - drag.startY,
        );
        if (deltaDist > 3) {
          drag.hasMoved = true;
        }

        if (drag.type === 'node' && drag.targetNode) {
          drag.targetNode.x = worldX;
          drag.targetNode.y = worldY;
          drag.targetNode.vx = 0;
          drag.targetNode.vy = 0;
          simulation.reheat(0.25);
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

      const hit = simulation.getNodeAt(worldX, worldY);
      if (hit !== hoveredNodeRef.current) {
        hoveredNodeRef.current = hit;
        setHoveredNode(hit);
        requestRender();
        if (hit) {
          setHoverScreenPos({
            x: hit.x * cam.zoom + cam.x,
            y: hit.y * cam.zoom + cam.y,
          });
        } else {
          setHoverScreenPos(null);
        }
      } else if (hit) {
        setHoverScreenPos({
          x: hit.x * cam.zoom + cam.x,
          y: hit.y * cam.zoom + cam.y,
        });
      }
    },
    [applyCamera, cameraRef, canvasRef, requestRender, simulationRef],
  );

  const handlePointerUp = useCallback(
    (e: ReactPointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      const simulation = simulationRef.current;
      if (canvas && canvas.hasPointerCapture(e.pointerId)) {
        canvas.releasePointerCapture(e.pointerId);
      }

      markInteracting(false);
      requestRender();

      const drag = dragStateRef.current;
      if (!drag) return;

      if (drag.type === 'node' && drag.targetNode) {
        drag.targetNode.pinned = false;
        simulation?.reheat(0.3);

        if (!drag.hasMoved) {
          const nextSelection =
            selectedEntityId === drag.targetNode.id ? null : drag.targetNode.id;
          onSelectEntity?.(nextSelection);
        }
      } else if (drag.type === 'pan') {
        if (!drag.hasMoved) {
          onSelectEntity?.(null);
        }
      }

      dragStateRef.current = null;
    },
    [
      canvasRef,
      markInteracting,
      onSelectEntity,
      requestRender,
      selectedEntityId,
      simulationRef,
    ],
  );

  const handlePointerLeave = useCallback(() => {
    if (hoveredNodeRef.current) {
      hoveredNodeRef.current = null;
      setHoveredNode(null);
      setHoverScreenPos(null);
      requestRender();
    }
  }, [requestRender]);

  const handleWheel = useCallback(
    (e: ReactWheelEvent<HTMLCanvasElement>) => {
      e.preventDefault();

      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;
      const zoomFactor = e.deltaY < 0 ? 1.15 : 0.87;
      zoomAtScreenPoint(mouseX, mouseY, zoomFactor);
    },
    [canvasRef, zoomAtScreenPoint],
  );

  return {
    hoveredNode,
    hoveredNodeRef,
    hoverScreenPos,
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
    handlePointerLeave,
    handleWheel,
  };
}
