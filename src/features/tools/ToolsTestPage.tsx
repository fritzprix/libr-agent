import { useState } from 'react';
import { useBuiltInTool } from './index';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { getLogger } from '@/lib/logger';
import { ToolCall } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';

const logger = getLogger('ToolsTestPage');

export function ToolsTestPage() {
  const { availableTools, executeTool, status } = useBuiltInTool();
  const [selectedTool, setSelectedTool] = useState<string>('');
  const [toolArguments, setToolArguments] = useState<string>('{}');
  const [executionResult, setExecutionResult] = useState<string>('');
  const [isExecuting, setIsExecuting] = useState(false);

  const handleExecuteTool = async () => {
    if (!selectedTool) return;

    const tool = availableTools.find((t) => t.name === selectedTool);
    if (!tool) return;

    setIsExecuting(true);
    setExecutionResult('');

    try {
      const toolCall: ToolCall = {
        id: createId(),
        type: 'function',
        function: {
          name: selectedTool,
          arguments: toolArguments,
        },
      };

      logger.info('Executing tool', { toolCall });
      const result = await executeTool(toolCall);
      setExecutionResult(JSON.stringify(result, null, 2));
      logger.info('Tool execution completed', { result });
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      setExecutionResult(`Error: ${errorMessage}`);
      logger.error('Tool execution failed', { error });
    } finally {
      setIsExecuting(false);
    }
  };

  const formatToolName = (name: string) => {
    if (name.startsWith('builtin.')) {
      const parts = name.replace('builtin.', '').split('__');
      return `${parts[0]} → ${parts.slice(1).join('__')}`;
    }
    return name;
  };

  const getServiceFromToolName = (name: string) => {
    if (name.startsWith('builtin.')) {
      return name.replace('builtin.', '').split('__')[0];
    }
    return 'unknown';
  };

  const groupedTools = availableTools.reduce(
    (groups, tool) => {
      const service = getServiceFromToolName(tool.name);
      if (!groups[service]) {
        groups[service] = [];
      }
      groups[service].push(tool);
      return groups;
    },
    {} as Record<string, typeof availableTools>,
  );

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="space-y-2">
        <h1 className="text-3xl font-bold tracking-tight">
          Built-in Tools Test Page
        </h1>
        <p className="text-muted-foreground">
          Test and explore the built-in tools available in the system
        </p>
      </div>

      <Tabs defaultValue="overview" className="space-y-4">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="execute">Execute Tools</TabsTrigger>
          <TabsTrigger value="services">Service Status</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Tool Statistics</CardTitle>
              <CardDescription>
                Overview of available tools by service
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="text-center p-4 bg-blue-50 rounded-lg">
                  <div className="text-2xl font-bold text-blue-600">
                    {availableTools.length}
                  </div>
                  <div className="text-sm text-blue-600">Total Tools</div>
                </div>
                <div className="text-center p-4 bg-green-50 rounded-lg">
                  <div className="text-2xl font-bold text-green-600">
                    {Object.keys(groupedTools).length}
                  </div>
                  <div className="text-sm text-green-600">Services</div>
                </div>
                <div className="text-center p-4 bg-purple-50 rounded-lg">
                  <div className="text-2xl font-bold text-purple-600">
                    {Object.values(status).filter((s) => s === 'ready').length}
                  </div>
                  <div className="text-sm text-purple-600">Ready Services</div>
                </div>
              </div>
            </CardContent>
          </Card>

          <div className="grid gap-4">
            {Object.entries(groupedTools).map(([service, tools]) => (
              <Card key={service}>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <CardTitle className="capitalize">
                      {service} Service
                    </CardTitle>
                    <Badge
                      variant={
                        status[service] === 'ready' ? 'default' : 'secondary'
                      }
                    >
                      {status[service] || 'unknown'}
                    </Badge>
                  </div>
                  <CardDescription>
                    {tools.length} tool{tools.length !== 1 ? 's' : ''} available
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="space-y-2">
                    {tools.map((tool) => (
                      <div
                        key={tool.name}
                        className="flex items-start justify-between p-3 border rounded-lg"
                      >
                        <div className="space-y-1">
                          <div className="font-medium text-sm">
                            {formatToolName(tool.name)}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            {tool.description}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </TabsContent>

        <TabsContent value="execute" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Execute Tool</CardTitle>
              <CardDescription>
                Select a tool and provide arguments to test execution
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Select Tool</label>
                <select
                  value={selectedTool}
                  onChange={(e) => setSelectedTool(e.target.value)}
                  className="w-full p-2 border rounded-md"
                >
                  <option value="">Select a tool...</option>
                  {availableTools.map((tool) => (
                    <option key={tool.name} value={tool.name}>
                      {formatToolName(tool.name)} - {tool.description}
                    </option>
                  ))}
                </select>
              </div>

              {selectedTool && (
                <div className="space-y-4">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">
                      Tool Arguments (JSON)
                    </label>
                    <Textarea
                      value={toolArguments}
                      onChange={(e) => setToolArguments(e.target.value)}
                      placeholder='{"key": "value"}'
                      className="font-mono"
                      rows={4}
                    />
                  </div>

                  <Button
                    onClick={handleExecuteTool}
                    disabled={isExecuting}
                    className="w-full"
                  >
                    {isExecuting ? 'Executing...' : 'Execute Tool'}
                  </Button>
                </div>
              )}

              {executionResult && (
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Execution Result
                  </label>
                  <pre className="p-4 bg-gray-100 border rounded-md overflow-auto text-xs">
                    {executionResult}
                  </pre>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="services" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Service Status</CardTitle>
              <CardDescription>
                Current status of all registered services
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {Object.entries(status).map(([serviceId, serviceStatus]) => (
                  <div
                    key={serviceId}
                    className="flex items-center justify-between p-4 border rounded-lg"
                  >
                    <div>
                      <div className="font-medium capitalize">{serviceId}</div>
                      <div className="text-sm text-muted-foreground">
                        {groupedTools[serviceId]?.length || 0} tools available
                      </div>
                    </div>
                    <Badge
                      variant={
                        serviceStatus === 'ready'
                          ? 'default'
                          : serviceStatus === 'loading'
                            ? 'secondary'
                            : serviceStatus === 'error'
                              ? 'destructive'
                              : 'outline'
                      }
                    >
                      {serviceStatus}
                    </Badge>
                  </div>
                ))}

                {Object.keys(status).length === 0 && (
                  <div className="text-center p-8 text-muted-foreground">
                    No services registered
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
