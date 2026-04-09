import type { FileNode } from './types';

function sortNodes(nodes: FileNode[]): FileNode[] {
  return [...nodes]
    .sort((left, right) => {
      if (left.isDirectory !== right.isDirectory) {
        return left.isDirectory ? -1 : 1;
      }

      return left.name.localeCompare(right.name, undefined, {
        sensitivity: 'base',
      });
    })
    .map((node) => ({
      ...node,
      children: node.children ? sortNodes(node.children) : node.children,
    }));
}

export function buildFileTreeFromPaths(paths: string[]): FileNode[] {
  const rootNodes: FileNode[] = [];

  for (const rawPath of paths) {
    const normalizedPath = rawPath
      .replace(/\\/g, '/')
      .replace(/^\/+|\/+$/g, '');

    if (!normalizedPath) {
      continue;
    }

    const segments = normalizedPath.split('/').filter(Boolean);
    let currentLevel = rootNodes;

    for (let index = 0; index < segments.length; index += 1) {
      const segment = segments[index];
      const nodePath = segments.slice(0, index + 1).join('/');
      const isDirectory = index < segments.length - 1;

      let node = currentLevel.find((candidate) => candidate.path === nodePath);

      if (!node) {
        node = {
          id: nodePath,
          name: segment,
          path: nodePath,
          isDirectory,
          isExpanded: false,
          children: isDirectory ? [] : undefined,
        };
        currentLevel.push(node);
      }

      if (isDirectory) {
        if (!node.children) {
          node.children = [];
        }
        currentLevel = node.children;
      }
    }
  }

  return sortNodes(rootNodes);
}
