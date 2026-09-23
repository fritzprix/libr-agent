import { describe, expect, it } from 'vitest';
import {
  computeFitCamera,
  zoomCameraAtPoint,
} from './useCanvasCamera';
import type { SimulationNode } from './knowledge-graph-types';

function makeNode(
  partial: Pick<SimulationNode, 'id' | 'x' | 'y'> &
    Partial<Omit<SimulationNode, 'id' | 'x' | 'y'>>,
): SimulationNode {
  return {
    name: `n${partial.id}`,
    isPrimary: false,
    vx: 0,
    vy: 0,
    radius: 20,
    color: '#fff',
    connectionCount: 0,
    ...partial,
  };
}

describe('computeFitCamera', () => {
  it('centers empty viewport when there are no nodes', () => {
    expect(computeFitCamera([], 800, 600)).toEqual({
      x: 400,
      y: 300,
      zoom: 1,
    });
  });

  it('fits a single node near the viewport center', () => {
    const camera = computeFitCamera([makeNode({ id: 1, x: 100, y: 50 })], 800, 600);
    expect(camera.zoom).toBeGreaterThan(0);
    expect(camera.zoom).toBeLessThanOrEqual(1.5);
    // Node world (100,50) should map near screen center after transform
    const screenX = 100 * camera.zoom + camera.x;
    const screenY = 50 * camera.zoom + camera.y;
    expect(screenX).toBeCloseTo(400, 0);
    expect(screenY).toBeCloseTo(300, 0);
  });
});

describe('zoomCameraAtPoint', () => {
  it('keeps the world point under the cursor stable', () => {
    const prev = { x: 100, y: 80, zoom: 1 };
    const screenX = 200;
    const screenY = 150;
    const next = zoomCameraAtPoint(prev, screenX, screenY, 2);
    const worldX = (screenX - prev.x) / prev.zoom;
    const worldY = (screenY - prev.y) / prev.zoom;
    expect((screenX - next.x) / next.zoom).toBeCloseTo(worldX);
    expect((screenY - next.y) / next.zoom).toBeCloseTo(worldY);
    expect(next.zoom).toBe(2);
  });

  it('clamps zoom to configured bounds', () => {
    const prev = { x: 0, y: 0, zoom: 1 };
    expect(zoomCameraAtPoint(prev, 10, 10, 100).zoom).toBe(3.5);
    expect(zoomCameraAtPoint(prev, 10, 10, 0.01).zoom).toBe(0.2);
  });
});
