import { describe, expect, it } from 'vitest';
import { ForceSimulation } from './knowledge-graph-simulation';
import type { SimulationLink, SimulationNode } from './knowledge-graph-types';

describe('ForceSimulation', () => {
  it('initializes and stabilizes nodes via alpha decay', () => {
    const sim = new ForceSimulation({
      alphaDecay: 0.1,
      alphaMin: 0.01,
      centerX: 0,
      centerY: 0,
    });

    const nodes: SimulationNode[] = [
      {
        id: 1,
        name: 'Node 1',
        isPrimary: true,
        x: 0,
        y: 0,
        vx: 0,
        vy: 0,
        radius: 20,
        color: '#6366f1',
        connectionCount: 1,
      },
      {
        id: 2,
        name: 'Node 2',
        isPrimary: false,
        x: 0,
        y: 0,
        vx: 0,
        vy: 0,
        radius: 14,
        color: '#38bdf8',
        connectionCount: 1,
      },
    ];

    sim.setNodes(nodes, false);
    expect(sim.nodes.length).toBe(2);

    const links: SimulationLink[] = [
      {
        id: 1,
        source: sim.nodes[0],
        target: sim.nodes[1],
        sourceId: 1,
        targetId: 2,
        relationType: 'relates_to',
        weight: 1,
      },
    ];

    sim.setLinks(links);

    // Initial tick should move nodes and decay alpha
    const initialAlpha = sim.getAlpha();
    expect(initialAlpha).toBeGreaterThan(0);

    let iterations = 0;
    while (!sim.isSettled() && iterations < 200) {
      sim.tick();
      iterations++;
    }

    expect(sim.isSettled()).toBe(true);
    expect(sim.getAlpha()).toBe(0);
    // Nodes should have moved apart
    const dx = sim.nodes[0].x - sim.nodes[1].x;
    const dy = sim.nodes[0].y - sim.nodes[1].y;
    const dist = Math.hypot(dx, dy);
    expect(dist).toBeGreaterThan(10);
  });

  it('supports hit testing with getNodeAt', () => {
    const sim = new ForceSimulation();
    const node: SimulationNode = {
      id: 42,
      name: 'Test Node',
      isPrimary: true,
      x: 100,
      y: 100,
      vx: 0,
      vy: 0,
      radius: 20,
      color: '#6366f1',
      connectionCount: 0,
    };
    sim.setNodes([node], false);

    // Click inside node radius
    const hit = sim.getNodeAt(105, 105);
    expect(hit).not.toBeNull();
    expect(hit?.id).toBe(42);

    // Click far away
    const miss = sim.getNodeAt(500, 500);
    expect(miss).toBeNull();
  });

  it('respects pinned nodes during simulation ticks', () => {
    const sim = new ForceSimulation({ alphaDecay: 0.05 });
    const pinnedNode: SimulationNode = {
      id: 10,
      name: 'Pinned Node',
      isPrimary: false,
      x: 50,
      y: 50,
      vx: 0,
      vy: 0,
      radius: 15,
      color: '#ffffff',
      connectionCount: 0,
      pinned: true,
    };

    sim.setNodes([pinnedNode], false);
    sim.tick();

    expect(sim.nodes[0].x).toBe(50);
    expect(sim.nodes[0].y).toBe(50);
    expect(sim.nodes[0].vx).toBe(0);
    expect(sim.nodes[0].vy).toBe(0);
  });

  it('rematches link source and target references when setNodes is refreshed', () => {
    const sim = new ForceSimulation();

    const initialNodes: SimulationNode[] = [
      {
        id: 1,
        name: 'Node A',
        isPrimary: true,
        x: 10,
        y: 10,
        vx: 0,
        vy: 0,
        radius: 20,
        color: '#6366f1',
        connectionCount: 1,
      },
      {
        id: 2,
        name: 'Node B',
        isPrimary: false,
        x: 20,
        y: 20,
        vx: 0,
        vy: 0,
        radius: 14,
        color: '#38bdf8',
        connectionCount: 1,
      },
    ];

    sim.setNodes(initialNodes, false);

    const initialLinks: SimulationLink[] = [
      {
        id: '1-2',
        source: initialNodes[0],
        target: initialNodes[1],
        sourceId: 1,
        targetId: 2,
        relationType: 'depends_on',
        weight: 1,
      },
    ];

    sim.setLinks(initialLinks);

    // Verify initial matching
    expect(sim.links[0].source).toBe(sim.findNodeById(1));
    expect(sim.links[0].target).toBe(sim.findNodeById(2));

    // Simulate refresh with brand new node objects
    const updatedNodes: SimulationNode[] = [
      {
        id: 1,
        name: 'Node A Updated',
        isPrimary: true,
        x: 15,
        y: 15,
        vx: 0,
        vy: 0,
        radius: 20,
        color: '#6366f1',
        connectionCount: 1,
      },
      {
        id: 2,
        name: 'Node B Updated',
        isPrimary: false,
        x: 25,
        y: 25,
        vx: 0,
        vy: 0,
        radius: 14,
        color: '#38bdf8',
        connectionCount: 1,
      },
    ];

    sim.setNodes(updatedNodes, true);
    sim.tick();

    const link = sim.links[0];
    expect(link.source).toBe(sim.findNodeById(link.source.id));
    expect(link.target).toBe(sim.findNodeById(link.target.id));
    expect(link.source).not.toBe(initialNodes[0]);
    expect(link.target).not.toBe(initialNodes[1]);
  });
});
