import React, { useState, useCallback } from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';

import {
  AlertCircle,
  Calculator,
  FileText,
  PlayCircle,
  RefreshCw,
} from 'lucide-react';
import { useWebMCPTools, useWebMCPManagement } from '@/hooks/use-web-mcp';
import { useWebMCPServer } from '@/context/WebMCPContext';
import { useUnifiedMCP } from '@/context/UnifiedMCPContext';
import { getLogger } from '@/lib/logger';
import { runWebMCPTests, TestResult } from '@/lib/web-mcp/test-integration';
import type { CalculatorServer } from '@/lib/web-mcp/modules/calculator';

const logger = getLogger('WebMCPDemo');

// 간단한 Calculator Direct Call Demo 컴포넌트
const CalculatorDirectDemo: React.FC = () => {
  const [calcA, setCalcA] = useState('5');
  const [calcB, setCalcB] = useState('3');
  const [results, setResults] = useState<Record<string, unknown>>({});
  const [loading, setLoading] = useState<Record<string, boolean>>({});

  // 타입 안전한 Calculator 서버 사용
  const {
    server,
    loading: serverLoading,
    error: serverError,
  } = useWebMCPServer('calculator');

  // 타입 캐스팅으로 Calculator 서버 메서드 사용
  const calculatorServer = server as CalculatorServer | null;

  const executeWithLoading = useCallback(
    async (key: string, fn: () => Promise<unknown>) => {
      setLoading((prev) => ({ ...prev, [key]: true }));
      try {
        const result = await fn();
        setResults((prev) => ({ ...prev, [key]: result }));
        logger.info(`Direct call completed: ${key}`, { result });
      } catch (error) {
        const errorResult = {
          error: error instanceof Error ? error.message : String(error),
          timestamp: new Date().toISOString(),
        };
        setResults((prev) => ({ ...prev, [key]: errorResult }));
        logger.error(`Direct call failed: ${key}`, error);
      } finally {
        setLoading((prev) => ({ ...prev, [key]: false }));
      }
    },
    [],
  );

  // Calculator 직접 호출 함수들
  const directAdd = () =>
    executeWithLoading('direct_add', async () => {
      if (!calculatorServer) throw new Error('Calculator server not ready');
      return await calculatorServer.add({ a: parseFloat(calcA), b: parseFloat(calcB) });
    });

  const directSubtract = () =>
    executeWithLoading('direct_subtract', async () => {
      if (!calculatorServer) throw new Error('Calculator server not ready');
      return await calculatorServer.subtract({ a: parseFloat(calcA), b: parseFloat(calcB) });
    });

  const directMultiply = () =>
    executeWithLoading('direct_multiply', async () => {
      if (!calculatorServer) throw new Error('Calculator server not ready');
      return await calculatorServer.multiply({ a: parseFloat(calcA), b: parseFloat(calcB) });
    });

  const directDivide = () =>
    executeWithLoading('direct_divide', async () => {
      if (!calculatorServer) throw new Error('Calculator server not ready');
      return await calculatorServer.divide({ a: parseFloat(calcA), b: parseFloat(calcB) });
    });

  const renderResult = (key: string) => {
    const result = results[key];
    if (!result) return null;

    return (
      <div className="mt-2 p-3 bg-muted rounded-md">
        <div className="text-sm font-medium mb-1">Result:</div>
        <pre className="text-xs overflow-auto whitespace-pre-wrap">
          {typeof result === 'string'
            ? result
            : JSON.stringify(result, null, 2)}
        </pre>
      </div>
    );
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Calculator className="h-5 w-5" />
          Calculator Direct Call Demo
        </CardTitle>
        <CardDescription>
          Call Calculator server methods directly with type safety
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center gap-2">
          <Badge variant={calculatorServer ? 'default' : 'secondary'}>
            {serverLoading ? 'Loading...' : calculatorServer ? 'Ready' : 'Not Ready'}
          </Badge>
          {serverError && (
            <div className="flex items-center gap-1 text-destructive text-sm">
              <AlertCircle className="h-4 w-4" />
              {serverError}
            </div>
          )}
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div>
            <Label htmlFor="directCalcA">Number A</Label>
            <Input
              id="directCalcA"
              type="number"
              value={calcA}
              onChange={(e) => setCalcA(e.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="directCalcB">Number B</Label>
            <Input
              id="directCalcB"
              type="number"
              value={calcB}
              onChange={(e) => setCalcB(e.target.value)}
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <Button
            onClick={directAdd}
            disabled={!calculatorServer || loading.direct_add}
            variant="outline"
            size="sm"
          >
            {loading.direct_add ? '...' : 'ADD'}
          </Button>
          <Button
            onClick={directSubtract}
            disabled={!calculatorServer || loading.direct_subtract}
            variant="outline"
            size="sm"
          >
            {loading.direct_subtract ? '...' : 'SUBTRACT'}
          </Button>
          <Button
            onClick={directMultiply}
            disabled={!calculatorServer || loading.direct_multiply}
            variant="outline"
            size="sm"
          >
            {loading.direct_multiply ? '...' : 'MULTIPLY'}
          </Button>
          <Button
            onClick={directDivide}
            disabled={!calculatorServer || loading.direct_divide}
            variant="outline"
            size="sm"
          >
            {loading.direct_divide ? '...' : 'DIVIDE'}
          </Button>
        </div>

        {['direct_add', 'direct_subtract', 'direct_multiply', 'direct_divide'].map((key) => 
          renderResult(key)
        )}
      </CardContent>
    </Card>
  );
};

export const WebMCPDemo: React.FC = () => {
  const [calcA, setCalcA] = useState('5');
  const [calcB, setCalcB] = useState('3');
  const [filePath, setFilePath] = useState('/tmp/test.txt');
  const [fileContent, setFileContent] = useState('Hello from Web MCP!');
  const [results, setResults] = useState<Record<string, unknown>>({});
  const [loading, setLoading] = useState<Record<string, boolean>>({});
  const [testResults, setTestResults] = useState<TestResult[]>([]);

  const {
    availableTools,
    executeCall,
    isReady,
    error: webMcpError,
  } = useWebMCPTools();

  const { loadServer } = useWebMCPManagement();

  const { systemStatus } = useUnifiedMCP();

  const executeWithLoading = useCallback(
    async (key: string, fn: () => Promise<unknown>) => {
      setLoading((prev) => ({ ...prev, [key]: true }));
      try {
        const result = await fn();
        setResults((prev) => ({ ...prev, [key]: result }));
        logger.info(`Demo execution completed: ${key}`, { result });
      } catch (error) {
        const errorResult = {
          error: error instanceof Error ? error.message : String(error),
          timestamp: new Date().toISOString(),
        };
        setResults((prev) => ({ ...prev, [key]: errorResult }));
        logger.error(`Demo execution failed: ${key}`, error);
      } finally {
        setLoading((prev) => ({ ...prev, [key]: false }));
      }
    },
    [],
  );

  const runTests = useCallback(() => {
    executeWithLoading('run_tests', async () => {
      const results = await runWebMCPTests();
      setTestResults(results);
      return {
        message: 'Tests completed',
        summary: `${results.filter((r) => r.success).length}/${results.length} passed`,
      };
    });
  }, [executeWithLoading]);

  const testCalculator = useCallback(
    (operation: string) => {
      const args =
        operation === 'factorial'
          ? { n: parseInt(calcA) }
          : { a: parseFloat(calcA), b: parseFloat(calcB) };

      executeWithLoading(`calc_${operation}`, () =>
        executeCall('calculator', operation, args),
      );
    },
    [calcA, calcB, executeCall, executeWithLoading],
  );

  const testFileSystem = useCallback(
    (operation: string) => {
      const args =
        operation === 'write_file'
          ? { path: filePath, content: fileContent }
          : operation === 'read_file'
            ? { path: filePath }
            : { path: filePath };

      executeWithLoading(`fs_${operation}`, () =>
        executeCall('filesystem', operation, args),
      );
    },
    [filePath, fileContent, executeCall, executeWithLoading],
  );

  const loadServers = useCallback(() => {
    executeWithLoading('load_servers', async () => {
      await loadServer('calculator');
      await loadServer('filesystem');
      return { message: 'Servers loaded successfully' };
    });
  }, [loadServer, executeWithLoading]);

  const getWebMCPTools = () => {
    return availableTools.filter(
      (tool) =>
        tool.name.startsWith('calculator__') ||
        tool.name.startsWith('filesystem__') ||
        tool.name.startsWith('file-store__'),
    );
  };

  const renderResult = (key: string) => {
    const result = results[key];
    if (!result) return null;

    return (
      <div className="mt-2 p-3 bg-muted rounded-md">
        <div className="text-sm font-medium mb-1">Result:</div>
        <pre className="text-xs overflow-auto whitespace-pre-wrap">
          {typeof result === 'string'
            ? result
            : JSON.stringify(result, null, 2)}
        </pre>
      </div>
    );
  };

  const webMcpTools = getWebMCPTools();

  return (
    <div className="p-6 max-w-6xl mx-auto space-y-6">
      <div className="text-center">
        <h1 className="text-3xl font-bold mb-2">Web MCP Demo</h1>
        <p className="text-muted-foreground">
          Test Web Worker-based MCP servers without Node.js/Python dependencies
        </p>
      </div>

      {/* Direct Call Demo */}
      <CalculatorDirectDemo />

      {/* System Status */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <RefreshCw className="h-5 w-5" />
            System Status
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="space-y-2">
              <Label>Web MCP Status</Label>
              <div className="flex items-center gap-2">
                <Badge variant={isReady ? 'default' : 'destructive'}>
                  {isReady ? 'Ready' : 'Not Ready'}
                </Badge>
                {webMcpError && (
                  <div className="flex items-center gap-1 text-destructive text-sm">
                    <AlertCircle className="h-4 w-4" />
                    {webMcpError}
                  </div>
                )}
              </div>
            </div>

            <div className="space-y-2">
              <Label>Available Tools</Label>
              <Badge variant="outline">
                {webMcpTools.length} Web MCP Tools
              </Badge>
            </div>

            <div className="space-y-2">
              <Label>System Integration</Label>
              <div className="space-y-1">
                <Badge
                  variant={
                    systemStatus.webMCP.initialized ? 'default' : 'destructive'
                  }
                >
                  Web MCP: {systemStatus.webMCP.toolCount} tools
                </Badge>
                <Badge
                  variant={
                    systemStatus.tauriMCP.connected ? 'default' : 'secondary'
                  }
                >
                  Tauri MCP: {systemStatus.tauriMCP.toolCount} tools
                </Badge>
              </div>
            </div>
          </div>

          <div className="flex gap-2">
            <Button
              onClick={loadServers}
              disabled={loading.load_servers}
              size="sm"
            >
              {loading.load_servers ? 'Loading...' : 'Load MCP Servers'}
            </Button>
          </div>

          {renderResult('load_servers')}
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Calculator Server Demo */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Calculator className="h-5 w-5" />
              Calculator Server
            </CardTitle>
            <CardDescription>
              Test arithmetic operations using Web Worker MCP
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label htmlFor="calcA">Number A</Label>
                <Input
                  id="calcA"
                  type="number"
                  value={calcA}
                  onChange={(e) => setCalcA(e.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="calcB">Number B</Label>
                <Input
                  id="calcB"
                  type="number"
                  value={calcB}
                  onChange={(e) => setCalcB(e.target.value)}
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-2">
              {['add', 'subtract', 'multiply', 'divide'].map((op) => (
                <Button
                  key={op}
                  onClick={() => testCalculator(op)}
                  disabled={loading[`calc_${op}`]}
                  variant="outline"
                  size="sm"
                >
                  {loading[`calc_${op}`] ? '...' : op.toUpperCase()}
                </Button>
              ))}
            </div>

            <div className="grid grid-cols-2 gap-2">
              <Button
                onClick={() => testCalculator('power')}
                disabled={loading.calc_power}
                variant="outline"
                size="sm"
              >
                {loading.calc_power ? '...' : 'POWER'}
              </Button>
              <Button
                onClick={() => testCalculator('sqrt')}
                disabled={loading.calc_sqrt}
                variant="outline"
                size="sm"
              >
                {loading.calc_sqrt ? '...' : 'SQRT (A)'}
              </Button>
            </div>

            <Button
              onClick={() => testCalculator('factorial')}
              disabled={loading.calc_factorial}
              variant="outline"
              size="sm"
              className="w-full"
            >
              {loading.calc_factorial ? '...' : 'FACTORIAL (A)'}
            </Button>

            {Object.keys(results)
              .filter((k) => k.startsWith('calc_'))
              .map((key) => (
                <div key={key}>
                  <div className="text-sm font-medium capitalize mb-1">
                    {key.replace('calc_', '')} Result:
                  </div>
                  {renderResult(key)}
                </div>
              ))}
          </CardContent>
        </Card>

        {/* File System Server Demo */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FileText className="h-5 w-5" />
              File System Server
            </CardTitle>
            <CardDescription>
              Test file operations using Tauri APIs via Web Worker MCP
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div>
              <Label htmlFor="filePath">File Path</Label>
              <Input
                id="filePath"
                value={filePath}
                onChange={(e) => setFilePath(e.target.value)}
                placeholder="/path/to/file.txt"
              />
            </div>

            <div>
              <Label htmlFor="fileContent">File Content</Label>
              <Textarea
                id="fileContent"
                value={fileContent}
                onChange={(e) => setFileContent(e.target.value)}
                placeholder="Content to write to file"
                rows={3}
              />
            </div>

            <div className="grid grid-cols-2 gap-2">
              <Button
                onClick={() => testFileSystem('write_file')}
                disabled={loading.fs_write_file}
                variant="outline"
                size="sm"
              >
                {loading.fs_write_file ? '...' : 'Write File'}
              </Button>
              <Button
                onClick={() => testFileSystem('read_file')}
                disabled={loading.fs_read_file}
                variant="outline"
                size="sm"
              >
                {loading.fs_read_file ? '...' : 'Read File'}
              </Button>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <Button
                onClick={() => testFileSystem('file_exists')}
                disabled={loading.fs_file_exists}
                variant="outline"
                size="sm"
              >
                {loading.fs_file_exists ? '...' : 'File Exists'}
              </Button>
              <Button
                onClick={() => testFileSystem('get_file_info')}
                disabled={loading.fs_get_file_info}
                variant="outline"
                size="sm"
              >
                {loading.fs_get_file_info ? '...' : 'File Info'}
              </Button>
            </div>

            {Object.keys(results)
              .filter((k) => k.startsWith('fs_'))
              .map((key) => (
                <div key={key}>
                  <div className="text-sm font-medium capitalize mb-1">
                    {key.replace('fs_', '').replace('_', ' ')} Result:
                  </div>
                  {renderResult(key)}
                </div>
              ))}
          </CardContent>
        </Card>
      </div>

      {/* Integration Tests */}
      <Card>
        <CardHeader>
          <CardTitle>Integration Tests</CardTitle>
          <CardDescription>
            Run the full Web MCP integration test suite.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button onClick={runTests} disabled={loading.run_tests}>
            {loading.run_tests ? 'Running Tests...' : 'Run Integration Tests'}
          </Button>
          {renderResult('run_tests')}
          {testResults.length > 0 && (
            <div className="mt-4 space-y-2">
              {testResults.map((result, index) => (
                <div
                  key={index}
                  className={`p-2 rounded-md ${result.success ? 'bg-green-100' : 'bg-red-100'}`}
                >
                  <div className="flex justify-between items-center">
                    <span
                      className={`font-medium ${result.success ? 'text-green-800' : 'text-red-800'}`}
                    >
                      {result.name}
                    </span>
                    <Badge variant={result.success ? 'default' : 'destructive'}>
                      {result.success ? 'PASS' : 'FAIL'}
                    </Badge>
                  </div>
                  <div className="text-xs text-muted-foreground">
                    Duration: {result.duration}ms
                  </div>
                  {!result.success && (
                    <div className="text-xs text-red-700 mt-1">
                      {result.error}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Available Tools */}
      <Card>
        <CardHeader>
          <CardTitle>Available Web MCP Tools</CardTitle>
          <CardDescription>
            Tools available from loaded Web Worker MCP servers
          </CardDescription>
        </CardHeader>
        <CardContent>
          {webMcpTools.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {webMcpTools.map((tool) => (
                <Card key={tool.name} className="p-3">
                  <div className="font-medium text-sm mb-1">{tool.name}</div>
                  <div className="text-xs text-muted-foreground mb-2">
                    {tool.description}
                  </div>
                  <Badge variant="outline" className="text-xs">
                    {Object.keys(tool.inputSchema.properties || {}).length}{' '}
                    params
                  </Badge>
                </Card>
              ))}
            </div>
          ) : (
            <div className="text-center text-muted-foreground py-8">
              <PlayCircle className="h-12 w-12 mx-auto mb-2 opacity-50" />
              <p>No Web MCP tools available</p>
              <p className="text-sm">Load MCP servers to see available tools</p>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default WebMCPDemo;
