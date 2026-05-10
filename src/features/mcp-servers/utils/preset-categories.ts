import {
  BarChart3,
  FileText,
  Image,
  type LucideIcon,
  Search,
  Sparkles,
  Wrench,
} from 'lucide-react';

export const PRESET_CATEGORY_ORDER = [
  'search',
  'ai',
  'devtools',
  'data',
  'documents',
  'creative',
  'other',
] as const;

export type PresetCategory = (typeof PRESET_CATEGORY_ORDER)[number];

export interface PresetCategoryMeta {
  labelKey: string;
  defaultLabel: string;
  descriptionKey: string;
  defaultDescription: string;
  icon: LucideIcon;
}

export const PRESET_CATEGORY_META: Record<PresetCategory, PresetCategoryMeta> =
  {
    search: {
      labelKey: 'mcpServer.categories.search',
      defaultLabel: 'Search',
      descriptionKey: 'mcpServer.categoryDescriptions.search',
      defaultDescription: 'Web search, research, and information retrieval.',
      icon: Search,
    },
    ai: {
      labelKey: 'mcpServer.categories.ai',
      defaultLabel: 'AI / LLM',
      descriptionKey: 'mcpServer.categoryDescriptions.ai',
      defaultDescription: 'Foundation models, generation, and inference tools.',
      icon: Sparkles,
    },
    devtools: {
      labelKey: 'mcpServer.categories.devtools',
      defaultLabel: 'Developer Tools',
      descriptionKey: 'mcpServer.categoryDescriptions.devtools',
      defaultDescription: 'Code, GitHub, docs, and agent workflow helpers.',
      icon: Wrench,
    },
    data: {
      labelKey: 'mcpServer.categories.data',
      defaultLabel: 'Data',
      descriptionKey: 'mcpServer.categoryDescriptions.data',
      defaultDescription: 'Financial, economic, and structured data sources.',
      icon: BarChart3,
    },
    documents: {
      labelKey: 'mcpServer.categories.documents',
      defaultLabel: 'Documents',
      descriptionKey: 'mcpServer.categoryDescriptions.documents',
      defaultDescription: 'Document and notebook tooling.',
      icon: FileText,
    },
    creative: {
      labelKey: 'mcpServer.categories.creative',
      defaultLabel: 'Creative',
      descriptionKey: 'mcpServer.categoryDescriptions.creative',
      defaultDescription: 'Image generation and creative media workflows.',
      icon: Image,
    },
    other: {
      labelKey: 'mcpServer.categories.other',
      defaultLabel: 'Other',
      descriptionKey: 'mcpServer.categoryDescriptions.other',
      defaultDescription: 'Presets without a recognized category.',
      icon: Wrench,
    },
  };

export function normalizePresetCategory(category?: string): PresetCategory {
  return PRESET_CATEGORY_ORDER.includes(category as PresetCategory)
    ? (category as PresetCategory)
    : 'other';
}
