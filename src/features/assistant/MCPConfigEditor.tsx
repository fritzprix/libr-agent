import { Badge, Button, StatusIndicator, Textarea } from '@/components/ui';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useCallback } from 'react';
import { toast } from 'sonner';
import { DEFAULT_MCP_CONFIG } from '../../context/AssistantContext';

interface MCPConfigEditorProps {
  mcpConfigText: string;
  onChange: (text: string) => void;
}

export default function MCPConfigEditor({
  mcpConfigText,
  onChange,
}: MCPConfigEditorProps) {
  const { status, isLoading: isCheckingStatus } = useMCPServer();

  const handleChange = (v: string) => {
    onChange(v);
  };

  const handleFormatJson = useCallback(() => {
    try {
      // Fix all types of unicode quotes before parsing
      const fixedJson = mcpConfigText
        .replace(/[""]/g, '"') // Left/right double quotation marks
        .replace(/['']/g, "'") // Left/right single quotation marks
        .replace(/[‚„]/g, '"') // Low quotation marks
        .replace(/[‹›]/g, "'") // Single angle quotation marks
        .replace(/[«»]/g, '"') // Double angle quotation marks
        .replace(/[\u201C\u201D]/g, '"') // Unicode left/right double quotes
        .replace(/[\u2018\u2019]/g, "'") // Unicode left/right single quotes
        .replace(/[\u201E\u201A]/g, '"') // Unicode low quotes
        .replace(/[\u2039\u203A]/g, "'"); // Unicode angle quotes

      const parsed = JSON.parse(fixedJson);
      const formatted = JSON.stringify(parsed, null, 2);
      onChange(formatted);
      toast.success('JSON이 올바르게 포맷되었습니다.');
    } catch {
      toast.error('유효하지 않은 JSON 형식입니다.');
    }
  }, [onChange, mcpConfigText]);

  return (
    <div>
      <div className="flex justify-between items-center mb-2">
        <label className="text-muted-foreground font-medium">
          MCP 설정 (JSON)
          <span className="text-xs text-muted-foreground ml-2">
            - 연결할 MCP 서버들을 설정합니다
          </span>
        </label>
        <div className="flex gap-2">
          <Button size="sm" variant="ghost" disabled={isCheckingStatus}>
            {isCheckingStatus ? '확인중...' : '서버 상태 확인'}
          </Button>
          <Button size="sm" variant="ghost" onClick={handleFormatJson}>
            Format JSON
          </Button>
        </div>
      </div>

      <Textarea
        value={mcpConfigText}
        onChange={(e) => handleChange(e.target.value)}
        className="h-48 font-mono text-sm"
        placeholder={JSON.stringify(DEFAULT_MCP_CONFIG, null, 2)}
      />

      <div className="text-xs text-muted-foreground mt-1">
        * mcpServers 형식만 사용됩니다. 빈 객체로 두면 MCP 서버를 사용하지
        않습니다.
      </div>

      {Object.keys(status).length > 0 && (
        <div className="mt-3 p-2 bg-muted rounded border border-muted">
          <div className="text-xs text-muted-foreground mb-2">
            에디터 서버 상태:
          </div>
          <div className="flex flex-wrap gap-2">
            {Object.entries(status).map(([serverName, isConnected]) => (
              <div
                key={serverName}
                className="flex items-center gap-1 text-xs px-2 py-1 rounded bg-muted"
              >
                <StatusIndicator
                  status={isConnected ? 'connected' : 'disconnected'}
                />
                <span className="text-foreground">{serverName}</span>
                <Badge variant={isConnected ? 'default' : 'destructive'}>
                  {isConnected ? 'OK' : 'NOK'}
                </Badge>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
