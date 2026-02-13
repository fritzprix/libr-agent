export interface FileNode {
  id: string;
  name: string;
  path: string;
  isDirectory: boolean;
  children?: FileNode[];
  isExpanded?: boolean;
  isLoading?: boolean;
  parent?: string;
}

export interface FileTreeNodeProps {
  node: FileNode;
  depth?: number;
  onToggle: (node: FileNode) => void;
  onOpen: (node: FileNode) => void;
}
