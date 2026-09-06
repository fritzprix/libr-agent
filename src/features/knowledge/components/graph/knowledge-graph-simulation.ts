import type { SimulationLink, SimulationNode } from './knowledge-graph-types';

export interface ForceSimulationOptions {
  repulsionStrength?: number;
  linkDistance?: number;
  linkStrength?: number;
  centerStrength?: number;
  damping?: number;
  alphaDecay?: number;
  alphaMin?: number;
  maxSpeed?: number;
  centerX?: number;
  centerY?: number;
}

export class ForceSimulation {
  public nodes: SimulationNode[] = [];
  public links: SimulationLink[] = [];

  private repulsionStrength: number;
  private linkDistance: number;
  private linkStrength: number;
  private centerStrength: number;
  private damping: number;
  private alphaDecay: number;
  private alphaMin: number;
  private maxSpeed: number;
  private centerX: number;
  private centerY: number;

  private alphaVal = 1.0;

  constructor(options: ForceSimulationOptions = {}) {
    this.repulsionStrength = options.repulsionStrength ?? 2200;
    this.linkDistance = options.linkDistance ?? 110;
    this.linkStrength = options.linkStrength ?? 0.07;
    this.centerStrength = options.centerStrength ?? 0.035;
    this.damping = options.damping ?? 0.84;
    this.alphaDecay = options.alphaDecay ?? 0.02;
    this.alphaMin = options.alphaMin ?? 0.005;
    this.maxSpeed = options.maxSpeed ?? 18;
    this.centerX = options.centerX ?? 0;
    this.centerY = options.centerY ?? 0;
  }

  public setCenter(cx: number, cy: number) {
    this.centerX = cx;
    this.centerY = cy;
  }

  public setNodes(nodes: SimulationNode[], preserveState = true) {
    if (!preserveState || this.nodes.length === 0) {
      this.nodes = nodes;
      this.initializeNodePositions();
      this.rematchLinks();
      this.reheat(1.0);
      return;
    }

    const existingMap = new Map<number, SimulationNode>();
    for (const n of this.nodes) {
      existingMap.set(n.id, n);
    }

    this.nodes = nodes.map((n, index) => {
      const existing = existingMap.get(n.id);
      if (existing) {
        return {
          ...n,
          x: existing.x,
          y: existing.y,
          vx: existing.vx,
          vy: existing.vy,
          pinned: existing.pinned,
        };
      }

      // Arrange new node in a radial offset from center
      const angle = (index / Math.max(nodes.length, 1)) * Math.PI * 2;
      const radius = 60 + Math.random() * 80;
      return {
        ...n,
        x: this.centerX + Math.cos(angle) * radius,
        y: this.centerY + Math.sin(angle) * radius,
        vx: 0,
        vy: 0,
      };
    });

    this.rematchLinks();
    this.reheat(0.6);
  }

  public setLinks(links: SimulationLink[]) {
    this.links = links;
    this.rematchLinks();
  }

  public rematchLinks(): void {
    const nodeMap = new Map<number, SimulationNode>();
    for (const n of this.nodes) {
      nodeMap.set(n.id, n);
    }

    const matchedLinks: SimulationLink[] = [];
    for (const link of this.links) {
      const sourceId = link.source?.id ?? link.sourceId;
      const targetId = link.target?.id ?? link.targetId;
      const sourceNode = nodeMap.get(sourceId);
      const targetNode = nodeMap.get(targetId);

      if (sourceNode && targetNode) {
        link.source = sourceNode;
        link.target = targetNode;
        matchedLinks.push(link);
      }
    }
    this.links = matchedLinks;
  }

  public initializeNodePositions() {
    const total = this.nodes.length;
    if (total === 0) return;

    this.nodes.forEach((node, index) => {
      if (node.x === 0 && node.y === 0) {
        const angle = (index / total) * Math.PI * 2;
        const dist = node.isPrimary ? 50 : 120 + (index % 4) * 25;
        node.x = this.centerX + Math.cos(angle) * dist;
        node.y = this.centerY + Math.sin(angle) * dist;
      }
      node.vx = 0;
      node.vy = 0;
    });
  }

  public reheat(alpha = 1.0) {
    this.alphaVal = Math.max(this.alphaVal, alpha);
  }

  public stop() {
    this.alphaVal = 0;
  }

  public isSettled(): boolean {
    return this.alphaVal <= this.alphaMin;
  }

  public getAlpha(): number {
    return this.alphaVal;
  }

  public tick(): boolean {
    if (this.alphaVal <= this.alphaMin) {
      this.alphaVal = 0;
      return false;
    }

    const nodes = this.nodes;
    const links = this.links;
    const nodeCount = nodes.length;
    const currentAlpha = this.alphaVal;

    // 1. Center gravity force
    for (let i = 0; i < nodeCount; i++) {
      const node = nodes[i];
      if (node.pinned) continue;

      const dx = this.centerX - node.x;
      const dy = this.centerY - node.y;
      node.vx += dx * this.centerStrength * currentAlpha;
      node.vy += dy * this.centerStrength * currentAlpha;
    }

    // 2. Coulomb Repulsion between pairs of nodes
    for (let i = 0; i < nodeCount; i++) {
      const nodeA = nodes[i];
      for (let j = i + 1; j < nodeCount; j++) {
        const nodeB = nodes[j];

        let dx = nodeB.x - nodeA.x;
        let dy = nodeB.y - nodeA.y;
        let distSq = dx * dx + dy * dy;

        if (distSq < 0.01) {
          dx = (Math.random() - 0.5) * 2;
          dy = (Math.random() - 0.5) * 2;
          distSq = dx * dx + dy * dy;
        }

        const dist = Math.sqrt(distSq);
        const effectiveDist = Math.max(dist, 24);

        const primaryMultiplier =
          (nodeA.isPrimary ? 1.5 : 1.0) * (nodeB.isPrimary ? 1.5 : 1.0);

        // Inverse distance repulsion
        const force =
          (this.repulsionStrength * primaryMultiplier * currentAlpha) /
          (effectiveDist * effectiveDist);

        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;

        if (!nodeA.pinned) {
          nodeA.vx -= fx;
          nodeA.vy -= fy;
        }
        if (!nodeB.pinned) {
          nodeB.vx += fx;
          nodeB.vy += fy;
        }
      }
    }

    // 3. Hooke's Spring Attraction along links
    const linkCount = links.length;
    for (let i = 0; i < linkCount; i++) {
      const link = links[i];
      const source = link.source;
      const target = link.target;

      let dx = target.x - source.x;
      let dy = target.y - source.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 0.001;

      // Adjust link distance by weight if present
      const targetDistance =
        this.linkDistance *
        (1 / Math.max(0.6, Math.min(1.8, link.weight || 1)));

      const displacement = dist - targetDistance;
      const force = displacement * this.linkStrength * currentAlpha;

      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;

      if (!source.pinned) {
        source.vx += fx * 0.5;
        source.vy += fy * 0.5;
      }
      if (!target.pinned) {
        target.vx -= fx * 0.5;
        target.vy -= fy * 0.5;
      }
    }

    // 4. Velocity damping & position update
    for (let i = 0; i < nodeCount; i++) {
      const node = nodes[i];
      if (node.pinned) {
        node.vx = 0;
        node.vy = 0;
        continue;
      }

      node.vx *= this.damping;
      node.vy *= this.damping;

      const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
      if (speed > this.maxSpeed) {
        node.vx = (node.vx / speed) * this.maxSpeed;
        node.vy = (node.vy / speed) * this.maxSpeed;
      }

      node.x += node.vx;
      node.y += node.vy;
    }

    // 5. Alpha decay
    this.alphaVal *= 1 - this.alphaDecay;
    if (this.alphaVal < this.alphaMin) {
      this.alphaVal = 0;
      return false;
    }

    return true;
  }

  public getNodeAt(
    worldX: number,
    worldY: number,
    hitPadding = 6,
  ): SimulationNode | null {
    // Check nodes in reverse order so topmost rendered node is hit first
    for (let i = this.nodes.length - 1; i >= 0; i--) {
      const node = this.nodes[i];
      const dx = worldX - node.x;
      const dy = worldY - node.y;
      const maxHitDist = node.radius + hitPadding;
      if (dx * dx + dy * dy <= maxHitDist * maxHitDist) {
        return node;
      }
    }
    return null;
  }

  public findNodeById(id: number): SimulationNode | undefined {
    return this.nodes.find((n) => n.id === id);
  }
}
