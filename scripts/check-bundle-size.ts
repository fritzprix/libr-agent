import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  evaluateBundleBudget,
  formatBytes,
  summarizeBundleAssets,
  type BundleBudget,
  type BundleAssetSize,
} from './lib/bundle-budget';

const rootDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const assetsDir = path.join(rootDir, 'dist', 'assets');
const budgetPath = path.join(rootDir, 'scripts', 'bundle-size-budget.json');
const reportPath = path.join(rootDir, 'dist', 'bundle-size-report.json');

function readBundleBudget(): BundleBudget {
  const content = fs.readFileSync(budgetPath, 'utf8');
  return JSON.parse(content) as BundleBudget;
}

function collectBundleAssets(): BundleAssetSize[] {
  if (!fs.existsSync(assetsDir)) {
    throw new Error(
      `Missing build assets directory at ${assetsDir}. Run "pnpm build" before "pnpm perf:bundle".`,
    );
  }

  return fs
    .readdirSync(assetsDir, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => {
      const fullPath = path.join(assetsDir, entry.name);
      return {
        name: entry.name,
        size: fs.statSync(fullPath).size,
      };
    });
}

function writeBundleReport(report: unknown) {
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
}

function main() {
  const budget = readBundleBudget();
  const summary = summarizeBundleAssets(collectBundleAssets());
  const violations = evaluateBundleBudget(summary, budget);

  const report = {
    generatedAt: new Date().toISOString(),
    summary: {
      totalBytes: summary.totalBytes,
      totalJsBytes: summary.totalJsBytes,
      totalCssBytes: summary.totalCssBytes,
      largestJsAsset: summary.largestJsAsset,
      largestCssAsset: summary.largestCssAsset,
    },
    budget,
    violations,
  };

  writeBundleReport(report);

  console.log('Bundle size summary');
  console.log(`- Total JS: ${formatBytes(summary.totalJsBytes)}`);
  console.log(`- Total CSS: ${formatBytes(summary.totalCssBytes)}`);
  console.log(
    `- Largest JS asset: ${summary.largestJsAsset?.name ?? 'n/a'} (${formatBytes(summary.largestJsAsset?.size ?? 0)})`,
  );
  console.log(
    `- Largest CSS asset: ${summary.largestCssAsset?.name ?? 'n/a'} (${formatBytes(summary.largestCssAsset?.size ?? 0)})`,
  );
  console.log(`- Report: ${path.relative(rootDir, reportPath)}`);

  if (violations.length === 0) {
    console.log('Bundle budget check passed.');
    return;
  }

  console.error('Bundle budget exceeded:');
  for (const violation of violations) {
    console.error(
      `- ${violation.metric}: actual ${formatBytes(violation.actual)} > limit ${formatBytes(violation.limit)}`,
    );
  }

  process.exitCode = 1;
}

main();
