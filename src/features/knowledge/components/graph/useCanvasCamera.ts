import { useCallback, useRef, type MutableRefObject } from 'react';
import type { CameraTransform, SimulationNode } from './knowledge-graph-types';

export interface ContainerSize {
  width: number;
  height: number;
}

/**
 * Compute a camera that fits `nodesToFit` into the given viewport.
 * Pure helper — unit-testable without mounting the canvas.
 */
export function computeFitCamera(
  nodesToFit: SimulationNode[],
  width: number,
  height: number,
): CameraTransform {
  if (nodesToFit.length === 0 || width <= 0 || height <= 0) {
    return {
      x: width > 0 ? width / 2 : 400,
      y: height > 0 ? height / 2 : 300,
      zoom: 1.0,
    };
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

  return {
    x: width / 2 - centerX * targetZoom,
    y: height / 2 - centerY * targetZoom,
    zoom: targetZoom,
  };
}

export function zoomCameraAtPoint(
  prev: CameraTransform,
  screenX: number,
  screenY: number,
  factor: number,
  minZoom = 0.2,
  maxZoom = 3.5,
): CameraTransform {
  const worldX = (screenX - prev.x) / prev.zoom;
  const worldY = (screenY - prev.y) / prev.zoom;
  const newZoom = Math.min(Math.max(prev.zoom * factor, minZoom), maxZoom);
  return {
    x: screenX - worldX * newZoom,
    y: screenY - worldY * newZoom,
    zoom: newZoom,
  };
}

/**
 * Camera is ref-backed so pan/zoom can update every frame without React re-renders.
 * Callers that need a reactive snapshot should read `cameraRef.current` inside rAF
 * (or subscribe separately).
 */
export function useCanvasCamera(requestRender: () => void): {
  cameraRef: MutableRefObject<CameraTransform>;
  applyCamera: (next: CameraTransform) => void;
  fitCameraToNodes: (
    nodesToFit: SimulationNode[],
    width: number,
    height: number,
  ) => void;
  hasInitialFitRef: MutableRefObject<boolean>;
  zoomByFactor: (factor: number, containerSize: ContainerSize) => void;
  zoomAtScreenPoint: (screenX: number, screenY: number, factor: number) => void;
} {
  const cameraRef = useRef<CameraTransform>({
    x: 400,
    y: 300,
    zoom: 1.0,
  });
  const hasInitialFitRef = useRef(false);

  const applyCamera = useCallback(
    (next: CameraTransform) => {
      cameraRef.current = next;
      requestRender();
    },
    [requestRender],
  );

  const fitCameraToNodes = useCallback(
    (nodesToFit: SimulationNode[], width: number, height: number) => {
      applyCamera(computeFitCamera(nodesToFit, width, height));
    },
    [applyCamera],
  );

  const zoomByFactor = useCallback(
    (factor: number, containerSize: ContainerSize) => {
      const { width, height } = containerSize;
      applyCamera(
        zoomCameraAtPoint(cameraRef.current, width / 2, height / 2, factor),
      );
    },
    [applyCamera],
  );

  const zoomAtScreenPoint = useCallback(
    (screenX: number, screenY: number, factor: number) => {
      applyCamera(
        zoomCameraAtPoint(cameraRef.current, screenX, screenY, factor),
      );
    },
    [applyCamera],
  );

  return {
    cameraRef,
    applyCamera,
    fitCameraToNodes,
    hasInitialFitRef,
    zoomByFactor,
    zoomAtScreenPoint,
  };
}
