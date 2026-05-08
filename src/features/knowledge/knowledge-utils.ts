import type { KnowledgeGraphEntity } from '@/lib/backend/knowledge';

const knowledgeDateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
});

export function formatTimestamp(timestamp: number): string {
  return knowledgeDateFormatter.format(new Date(timestamp));
}

export function getKnowledgeCardTitle(
  preview: string,
  untitledLabel: string,
): string {
  const normalizedPreview = preview.replace(/\s+/g, ' ').trim();
  if (!normalizedPreview) {
    return untitledLabel;
  }

  const sentenceMatch = normalizedPreview.match(/^(.{1,96}?[.!?])(?:\s|$)/);
  if (sentenceMatch?.[1]) {
    return sentenceMatch[1];
  }

  if (normalizedPreview.length <= 96) {
    return normalizedPreview;
  }

  return `${normalizedPreview.slice(0, 93)}...`;
}

export function layoutGraphNodes(entities: KnowledgeGraphEntity[]) {
  const centerX = 180;
  const centerY = 140;
  const primary = entities.filter((entity) => entity.isPrimary);
  const secondary = entities.filter((entity) => !entity.isPrimary);
  const positions = new Map<number, { x: number; y: number }>();

  if (primary.length === 1) {
    positions.set(primary[0].id, { x: centerX, y: centerY });
  } else {
    primary.forEach((entity, index) => {
      const angle = (Math.PI * 2 * index) / Math.max(primary.length, 1);
      positions.set(entity.id, {
        x: centerX + Math.cos(angle) * 68,
        y: centerY + Math.sin(angle) * 68,
      });
    });
  }

  secondary.forEach((entity, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(secondary.length, 1);
    positions.set(entity.id, {
      x: centerX + Math.cos(angle) * 118,
      y: centerY + Math.sin(angle) * 118,
    });
  });

  return positions;
}
