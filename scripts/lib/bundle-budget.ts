export interface BundleAssetSize {
  name: string;
  size: number;
}

export interface BundleAssetStat extends BundleAssetSize {
  type: 'js' | 'css' | 'other';
}

export interface BundleBudget {
  totalCssBytes: number;
  largestJsBytes: number;
  largestCssBytes: number;
}

export interface BundleBudgetViolation {
  metric: keyof BundleBudget;
  actual: number;
  limit: number;
}

export interface BundleSummary {
  totalBytes: number;
  totalJsBytes: number;
  totalCssBytes: number;
  assets: BundleAssetStat[];
  largestJsAsset: BundleAssetStat | null;
  largestCssAsset: BundleAssetStat | null;
}

function classifyBundleAsset(name: string): BundleAssetStat['type'] {
  if (name.endsWith('.js')) {
    return 'js';
  }

  if (name.endsWith('.css')) {
    return 'css';
  }

  return 'other';
}

export function summarizeBundleAssets(
  assets: BundleAssetSize[],
): BundleSummary {
  const typedAssets = assets.map((asset) => ({
    ...asset,
    type: classifyBundleAsset(asset.name),
  }));

  let totalBytes = 0;
  let totalJsBytes = 0;
  let totalCssBytes = 0;
  let largestJsAsset: BundleAssetStat | null = null;
  let largestCssAsset: BundleAssetStat | null = null;

  for (const asset of typedAssets) {
    totalBytes += asset.size;

    if (asset.type === 'js') {
      totalJsBytes += asset.size;
      if (!largestJsAsset || asset.size > largestJsAsset.size) {
        largestJsAsset = asset;
      }
    }

    if (asset.type === 'css') {
      totalCssBytes += asset.size;
      if (!largestCssAsset || asset.size > largestCssAsset.size) {
        largestCssAsset = asset;
      }
    }
  }

  return {
    totalBytes,
    totalJsBytes,
    totalCssBytes,
    assets: typedAssets,
    largestJsAsset,
    largestCssAsset,
  };
}

export function evaluateBundleBudget(
  summary: BundleSummary,
  budget: BundleBudget,
): BundleBudgetViolation[] {
  const metrics: Array<[keyof BundleBudget, number]> = [
    ['totalCssBytes', summary.totalCssBytes],
    ['largestJsBytes', summary.largestJsAsset?.size ?? 0],
    ['largestCssBytes', summary.largestCssAsset?.size ?? 0],
  ];

  return metrics.flatMap(([metric, actual]) => {
    const limit = budget[metric];
    if (actual <= limit) {
      return [];
    }

    return [{ metric, actual, limit }];
  });
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const kib = bytes / 1024;
  if (kib < 1024) {
    return `${kib.toFixed(1)} KiB`;
  }

  return `${(kib / 1024).toFixed(2)} MiB`;
}
