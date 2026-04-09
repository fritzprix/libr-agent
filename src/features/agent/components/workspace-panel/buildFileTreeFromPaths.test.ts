import { describe, expect, it } from 'vitest';

import { buildFileTreeFromPaths } from './buildFileTreeFromPaths';

describe('buildFileTreeFromPaths', () => {
  it('builds a nested tree from relative file paths', () => {
    const tree = buildFileTreeFromPaths([
      'src/app/main.tsx',
      'src/components/Button.tsx',
      'README.md',
    ]);

    expect(tree).toEqual([
      {
        id: 'src',
        name: 'src',
        path: 'src',
        isDirectory: true,
        isExpanded: false,
        children: [
          {
            id: 'src/app',
            name: 'app',
            path: 'src/app',
            isDirectory: true,
            isExpanded: false,
            children: [
              {
                id: 'src/app/main.tsx',
                name: 'main.tsx',
                path: 'src/app/main.tsx',
                isDirectory: false,
                isExpanded: false,
                children: undefined,
              },
            ],
          },
          {
            id: 'src/components',
            name: 'components',
            path: 'src/components',
            isDirectory: true,
            isExpanded: false,
            children: [
              {
                id: 'src/components/Button.tsx',
                name: 'Button.tsx',
                path: 'src/components/Button.tsx',
                isDirectory: false,
                isExpanded: false,
                children: undefined,
              },
            ],
          },
        ],
      },
      {
        id: 'README.md',
        name: 'README.md',
        path: 'README.md',
        isDirectory: false,
        isExpanded: false,
        children: undefined,
      },
    ]);
  });

  it('normalizes separators and sorts directories before files', () => {
    const tree = buildFileTreeFromPaths([
      'zeta.txt',
      'docs\\guide.md',
      'docs/api/reference.md',
      '/alpha.txt/',
    ]);

    expect(tree.map((node) => node.path)).toEqual([
      'docs',
      'alpha.txt',
      'zeta.txt',
    ]);
    expect(tree[0].children?.map((node) => node.path)).toEqual([
      'docs/api',
      'docs/guide.md',
    ]);
  });
});
