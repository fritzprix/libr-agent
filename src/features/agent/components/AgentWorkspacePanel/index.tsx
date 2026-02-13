import { useRef } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Folder,
  RefreshCw,
  Upload,
  Terminal,
  AlertTriangle,
} from 'lucide-react';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { FileTreeNode } from './FileTreeNode';
import { useFileTree } from './useFileTree';
import { useWorkspaceOverride } from './useWorkspaceOverride';
import { useFileDrop } from './useFileDrop';
import { useWorkspaceActions } from './useWorkspaceActions';

export function AgentWorkspacePanel() {
  const { session } = useAgentSessionState();
  const panelRef = useRef<HTMLDivElement>(null);

  const {
    rootPath,
    fileTree,
    loading,
    error,
    loadDirectory,
    toggleDirectory,
  } = useFileTree();

  const {
    workspaceOverride,
    isOverrideActive,
    handleSetOverride,
    handleCancelOverride,
    handleBrowseFolder,
  } = useWorkspaceOverride(() => loadDirectory('./'));

  const { dragState } = useFileDrop(rootPath, () => loadDirectory(rootPath), panelRef);

  const {
    handleOpenInExplorer,
    handleOpenInTerminal,
    handleOpenFile,
  } = useWorkspaceActions();

  if (!session) return null;

  return (
    <div
      ref={panelRef}
      className={`w-80 h-full ${dragState.isOver ? 'ring-2 ring-success' : ''}`}
    >
      <Card
        className={`w-full h-full flex flex-col bg-background/95 backdrop-blur border-border/50 ${
          dragState.isOver ? 'border-success bg-success/10' : ''
        }`}
      >
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <Folder className="w-4 h-4" />
              Workspace Files
            </CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleOpenInExplorer}
                className="h-6 px-2 text-xs"
                title="Open in Explorer"
              >
                <Folder className="w-3 h-3" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleOpenInTerminal}
                className="h-6 px-2 text-xs"
                title="Open in Terminal"
              >
                <Terminal className="w-3 h-3" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => loadDirectory(rootPath)}
                className="h-6 w-6 p-0"
                title="Refresh"
              >
                <RefreshCw
                  className={`w-3 h-3 ${loading ? 'animate-spin' : ''}`}
                />
              </Button>
            </div>
          </div>

          {/* Workspace Override UI */}
          <div className="px-0 py-2 space-y-2 border-b border-border/50 mb-2">
            <div className="flex gap-2">
              <Input
                type="text"
                placeholder="No workspace override (click Browse to select)"
                value={workspaceOverride}
                readOnly
                className="h-7 text-xs flex-1"
                disabled={isOverrideActive}
              />
              {!isOverrideActive && (
                <Button
                  onClick={handleBrowseFolder}
                  size="sm"
                  variant="outline"
                  className="h-7 text-xs whitespace-nowrap"
                >
                  Browse...
                </Button>
              )}
              {!isOverrideActive ? (
                <Button
                  onClick={handleSetOverride}
                  size="sm"
                  className="h-7 text-xs"
                  disabled={!workspaceOverride.trim()}
                >
                  Set
                </Button>
              ) : (
                <Button
                  onClick={handleCancelOverride}
                  size="sm"
                  variant="destructive"
                  className="h-7 text-xs"
                >
                  Cancel
                </Button>
              )}
            </div>
            {isOverrideActive && (
              <p className="text-xs text-warning flex items-center gap-1">
                <AlertTriangle className="w-3 h-3" />
                Using custom workspace
              </p>
            )}
          </div>

          <div
            className="text-xs text-muted-foreground truncate"
            title={rootPath}
          >
            {rootPath}
          </div>
        </CardHeader>

        <CardContent className="flex-1 overflow-auto px-0">
          {error && (
            <div className="text-xs text-destructive p-2 mx-2 rounded bg-destructive/10">
              {error}
            </div>
          )}

          {loading && fileTree.length === 0 ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="w-4 h-4 animate-spin mr-2" />
              <span className="text-xs text-muted-foreground">Loading...</span>
            </div>
          ) : (
            <div className="space-y-0">
              {fileTree.map((node) => (
                <FileTreeNode
                  key={node.id}
                  node={node}
                  onToggle={toggleDirectory}
                  onOpen={handleOpenFile}
                />
              ))}

              {fileTree.length === 0 && !loading && (
                <div className="text-xs text-muted-foreground text-center py-8">
                  No files found
                </div>
              )}
            </div>
          )}
        </CardContent>

        <div className="border-2 border-dashed border-muted-foreground/25 rounded m-2 p-2 text-center text-xs text-muted-foreground hover:border-muted-foreground/50 transition-colors">
          <Upload className="w-4 h-4 mx-auto mb-1" />
          Drop files here to upload
        </div>
      </Card>
    </div>
  );
}
