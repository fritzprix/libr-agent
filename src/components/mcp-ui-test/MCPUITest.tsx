import React, { useState, useCallback, useEffect } from 'react';
import {
  UIResourceRenderer,
  UIActionResult,
  basicComponentLibrary,
} from '@mcp-ui/client';
import { mockMCPServer, MockUIResource } from './MockMCPServer';
import { Button } from '../ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '../ui/card';
import { Badge } from '../ui/badge';
import { AlertCircle, CheckCircle, Info, ExternalLink } from 'lucide-react';

interface UIAction {
  id: string;
  timestamp: Date;
  type: string;
  payload: Record<string, unknown>;
  result?: string;
}

export const MCPUITest: React.FC = () => {
  const [currentResourceIndex, setCurrentResourceIndex] = useState(0);
  const [actions, setActions] = useState<UIAction[]>([]);
  const [isRendering, setIsRendering] = useState(false);

  const resources = mockMCPServer.getAllResources();
  const currentResource = resources[currentResourceIndex];

  // UI Action 핸들러
  const handleUIAction = useCallback(
    async (result: UIActionResult): Promise<{ status: string }> => {
      const actionId = `action-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

      const newAction: UIAction = {
        id: actionId,
        timestamp: new Date(),
        type: result.type,
        payload: result.payload,
      };

      setActions((prev) => [newAction, ...prev.slice(0, 9)]); // 최대 10개까지만 유지

      // 액션 타입에 따른 처리
      switch (result.type) {
        case 'tool':
          console.log(
            'Tool call:',
            result.payload.toolName,
            result.payload.params,
          );
          newAction.result = `Tool '${result.payload.toolName}' executed successfully`;
          break;
        case 'prompt':
          console.log('Prompt:', result.payload.prompt);
          newAction.result = 'Prompt displayed to user';
          break;
        case 'link':
          console.log('Link:', result.payload.url);
          newAction.result = `Link navigation to ${result.payload.url}`;
          break;
        case 'intent':
          console.log('Intent:', result.payload.intent, result.payload.params);
          newAction.result = `Intent '${result.payload.intent}' processed`;
          break;
        case 'notify':
          console.log('Notification:', result.payload.message);
          newAction.result = `Notification: ${result.payload.message}`;
          break;
      }

      // 상태 업데이트 (result 포함)
      setActions((prev) => [newAction, ...prev.slice(1)]);

      return { status: 'handled' };
    },
    [],
  );

  // 다음 리소스로 이동
  const nextResource = () => {
    if (currentResourceIndex < resources.length - 1) {
      setCurrentResourceIndex((prev) => prev + 1);
      setIsRendering(true);
      setTimeout(() => setIsRendering(false), 500);
    }
  };

  // 이전 리소스로 이동
  const prevResource = () => {
    if (currentResourceIndex > 0) {
      setCurrentResourceIndex((prev) => prev - 1);
      setIsRendering(true);
      setTimeout(() => setIsRendering(false), 500);
    }
  };
  useEffect(() => {
    const container = document.querySelector(
      '[data-testid="standard-dom-renderer-container"]',
    );
    if (container) {
      console.log('Found RemoteDOMRenderer container:', container);
      console.log('Container content:', container.innerHTML);

      // 강제로 내용 추가 테스트
      const testDiv = document.createElement('div');
      testDiv.textContent = 'TEST CONTENT FROM OUTSIDE';
      testDiv.style.background = 'red';
      testDiv.style.padding = '10px';
      container.appendChild(testDiv);
    } else {
      console.error('RemoteDOMRenderer container not found!');
    }
  }, [currentResourceIndex]);

  // 특정 리소스로 이동
  const goToResource = (index: number) => {
    setCurrentResourceIndex(index);
    setIsRendering(true);
    setTimeout(() => setIsRendering(false), 500);
  };

  // 액션 로그 클리어
  const clearActions = () => {
    setActions([]);
  };

  // 리소스 타입에 따른 배지 색상
  const getResourceTypeBadge = (resource: MockUIResource) => {
    const mimeType = resource.resource.resource.mimeType;
    if (mimeType === 'text/html') {
      return <Badge variant="default">HTML</Badge>;
    } else if (mimeType === 'text/uri-list') {
      return <Badge variant="secondary">URL</Badge>;
    } else if (mimeType?.includes('remote-dom')) {
      return <Badge variant="outline">Remote DOM</Badge>;
    }
    return <Badge variant="destructive">Unknown</Badge>;
  };

  // 액션 타입에 따른 아이콘
  const getActionIcon = (type: string) => {
    switch (type) {
      case 'tool':
        return <CheckCircle className="h-4 w-4 text-green-500" />;
      case 'notify':
        return <Info className="h-4 w-4 text-blue-500" />;
      case 'link':
        return <ExternalLink className="h-4 w-4 text-purple-500" />;
      default:
        return <AlertCircle className="h-4 w-4 text-orange-500" />;
    }
  };

  return (
    <div className="container mx-auto p-6 max-w-7xl">
      <div className="mb-8">
        <h1 className="text-3xl font-bold mb-2">
          MCP UI Resource Renderer Test
        </h1>
        <p className="text-muted-foreground">
          다양한 UIResource 타입들을 테스트하고 UIResourceRenderer의 동작을
          확인합니다.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* 리소스 리스트 패널 */}
        <div className="lg:col-span-1">
          <Card>
            <CardHeader>
              <CardTitle>Available Resources</CardTitle>
              <CardDescription>
                총 {resources.length}개의 테스트 리소스
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-2">
              {resources.map((resource, index) => (
                <div
                  key={resource.id}
                  className={`p-3 rounded-lg border cursor-pointer transition-all ${
                    index === currentResourceIndex
                      ? 'border-primary bg-primary/5'
                      : 'border-border hover:border-primary/50'
                  }`}
                  onClick={() => goToResource(index)}
                >
                  <div className="flex items-center justify-between mb-2">
                    <span className="font-medium text-sm">
                      {resource.title}
                    </span>
                    {getResourceTypeBadge(resource)}
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {resource.description}
                  </p>
                  <p className="text-xs text-muted-foreground mt-1 font-mono">
                    {resource.resource.resource.uri}
                  </p>
                </div>
              ))}
            </CardContent>
          </Card>

          {/* 액션 로그 */}
          <Card className="mt-6">
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>Action Log</CardTitle>
                <Button variant="outline" size="sm" onClick={clearActions}>
                  Clear
                </Button>
              </div>
              <CardDescription>
                UIResourceRenderer에서 발생한 액션들
              </CardDescription>
            </CardHeader>
            <CardContent>
              {actions.length === 0 ? (
                <p className="text-sm text-muted-foreground text-center py-4">
                  아직 액션이 없습니다
                </p>
              ) : (
                <div className="space-y-2 max-h-60 overflow-y-auto">
                  {actions.map((action) => (
                    <div key={action.id} className="p-2 rounded border text-xs">
                      <div className="flex items-center gap-2 mb-1">
                        {getActionIcon(action.type)}
                        <span>{JSON.stringify(action)}</span>
                        <span className="font-medium">{action.type}</span>
                        <span className="text-muted-foreground">
                          {action.timestamp.toLocaleTimeString()}
                        </span>
                      </div>
                      {action.result && (
                        <p className="text-muted-foreground">{action.result}</p>
                      )}
                      <details className="mt-1">
                        <summary className="cursor-pointer text-muted-foreground">
                          Payload
                        </summary>
                        <pre className="mt-1 p-1 bg-muted rounded text-xs overflow-x-auto">
                          {JSON.stringify(action.payload, null, 2)}
                        </pre>
                      </details>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        {/* 렌더러 패널 */}
        <div className="lg:col-span-2">
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <span>Current Resource</span>
                    {isRendering && (
                      <div className="animate-spin h-4 w-4 border-2 border-primary border-t-transparent rounded-full" />
                    )}
                  </CardTitle>
                  <CardDescription>
                    {currentResource?.title} - {currentResource?.description}
                  </CardDescription>
                </div>
                {getResourceTypeBadge(currentResource)}
              </div>
            </CardHeader>
            <CardContent>
              {/* 네비게이션 버튼 */}
              <div className="flex justify-between items-center mb-4">
                <Button
                  onClick={prevResource}
                  disabled={currentResourceIndex === 0}
                  variant="outline"
                >
                  Previous
                </Button>
                <span className="text-sm text-muted-foreground">
                  {currentResourceIndex + 1} / {resources.length}
                </span>
                <Button
                  onClick={nextResource}
                  disabled={currentResourceIndex === resources.length - 1}
                  variant="outline"
                >
                  Next
                </Button>
              </div>

              {/* UIResourceRenderer */}
              <div className="border rounded-lg overflow-hidden">
                {currentResource && (
                  <UIResourceRenderer
                    resource={currentResource.resource.resource}
                    onUIAction={handleUIAction}
                    remoteDomProps={{
                      library: basicComponentLibrary,
                    }}
                  />
                )}
              </div>

              {/* 리소스 정보 */}
              <div className="mt-4 p-4 bg-muted rounded-lg">
                <h4 className="font-medium mb-2">Resource Information</h4>
                <div className="grid grid-cols-2 gap-4 text-sm">
                  <div>
                    <span className="font-medium">URI:</span>
                    <p className="font-mono text-muted-foreground">
                      {currentResource?.resource.resource.uri}
                    </p>
                  </div>
                  <div>
                    <span className="font-medium">MIME Type:</span>
                    <p className="font-mono text-muted-foreground">
                      {currentResource?.resource.resource.mimeType}
                    </p>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
};
