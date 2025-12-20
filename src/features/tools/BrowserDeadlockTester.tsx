import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { getLogger } from '@/lib/logger';
import { useBrowserInvoker } from '@/hooks/use-browser-invoker';
import {
  createBrowserSession,
  closeBrowserSession,
} from '@/lib/backend/browser';
import { clickElementTool } from '@/features/tools/browser-tools/ClickElementTool';
import { inputTextTool } from '@/features/tools/browser-tools/InputTextTool';
import { extractWebContentTool } from '@/features/tools/browser-tools/ExtractContentTool';

const logger = getLogger('BrowserDeadlockTester');

export function BrowserDeadlockTester() {
  const [sessionId, setSessionId] = useState<string>('');
  const [url, setUrl] = useState('https://example.com');
  const [logs, setLogs] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const { executeScript } = useBrowserInvoker();

  const addLog = (msg: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [`[${timestamp}] ${msg}`, ...prev]);
    logger.info(msg);
  };

  const handleCreateSession = async () => {
    try {
      setIsRunning(true);
      addLog(`Creating session for ${url}...`);
      const result = await createBrowserSession({ url });
      setSessionId(result.session_id);
      addLog(`Session created: ${result.session_id}`);
      addLog(`Message: ${result.message}`);
    } catch (error) {
      addLog(`Error creating session: ${error}`);
    } finally {
      setIsRunning(false);
    }
  };

  const handleCloseSession = async () => {
    if (!sessionId) return;
    try {
      addLog(`Closing session ${sessionId}...`);
      await closeBrowserSession(sessionId);
      setSessionId('');
      addLog('Session closed');
    } catch (error) {
      addLog(`Error closing session: ${error}`);
    }
  };

  const runStressTest = async () => {
    if (!sessionId) {
      addLog('No active session');
      return;
    }

    setIsRunning(true);
    addLog('Starting stress test (Deadlock Reproduction)...');

    try {
      // 1. Extract Content (Should work immediately or fail fast)
      addLog('1. Testing ExtractContent...');
      const extractResult = await extractWebContentTool.execute(
        { sessionId, saveRawHtml: false },
        executeScript,
      );
      addLog(
        `Extract Result: ${JSON.stringify(extractResult).substring(0, 100)}...`,
      );

      // 2. Input Text (if input exists, or just try to inject)
      addLog('2. Testing InputText (injecting into body as test)...');
      // We'll try to input into a non-existent element to test error handling,
      // or a generic one if we are on example.com (it has no inputs)
      const inputResult = await inputTextTool.execute(
        { sessionId, selector: 'body', text: 'test' },
        executeScript,
      );
      addLog(`Input Result: ${JSON.stringify(inputResult)}`);

      // 3. Click Element (Try to click h1)
      addLog('3. Testing ClickElement (clicking h1)...');
      const clickResult = await clickElementTool.execute(
        { sessionId, selector: 'h1', waitForNavigation: 'auto' },
        executeScript,
      );
      addLog(`Click Result: ${JSON.stringify(clickResult)}`);
    } catch (error) {
      addLog(`Stress test failed: ${error}`);
    } finally {
      setIsRunning(false);
      addLog('Stress test finished');
    }
  };

  const runWikipediaTest = async () => {
    if (sessionId) {
      await handleCloseSession();
    }

    setIsRunning(true);
    addLog('Starting Wikipedia Navigation Test...');

    try {
      // 1. Open Wikipedia
      addLog('1. Opening https://www.wikipedia.org...');
      const sessionResult = await createBrowserSession({
        url: 'https://www.wikipedia.org',
      });
      const newSessionId = sessionResult.session_id;
      setSessionId(newSessionId);
      addLog(`Session created: ${newSessionId}`);

      // 2. Input "Tauri"
      addLog('2. Inputting "Tauri" into search box...');
      const inputResult = await inputTextTool.execute(
        { sessionId: newSessionId, selector: '#searchInput', text: 'Tauri' },
        executeScript,
      );
      addLog(`Input Result: ${JSON.stringify(inputResult)}`);

      // 3. Click Search (triggers navigation)
      addLog('3. Clicking Search button (expecting navigation)...');
      const clickResult = await clickElementTool.execute(
        {
          sessionId: newSessionId,
          selector: 'button[type="submit"]',
          waitForNavigation: true,
        },
        executeScript,
      );
      addLog(`Click Result: ${JSON.stringify(clickResult)}`);

      // 4. Extract Content from new page
      addLog('4. Extracting content from result page...');
      const extractResult = await extractWebContentTool.execute(
        { sessionId: newSessionId, saveRawHtml: false },
        executeScript,
      );
      const contentItem = extractResult.result?.content?.[0];
      const content = contentItem?.type === 'text' ? contentItem.text : '';
      const preview = content.substring(0, 200);
      addLog(`Extract Result (Preview): ${preview}...`);

      if (content.includes('Rust') || content.includes('framework')) {
        addLog('✅ SUCCESS: Found expected content on result page.');
      } else {
        addLog('⚠️ WARNING: Did not find expected keywords in content.');
      }
    } catch (error) {
      addLog(`Wikipedia test failed: ${error}`);
    } finally {
      setIsRunning(false);
      addLog('Wikipedia test finished');
    }
  };

  const runHuggingFaceTest = async () => {
    if (sessionId) {
      await handleCloseSession();
    }

    setIsRunning(true);
    addLog('Starting HuggingFace Test (Complex SPA)...');

    try {
      // 1. Open HuggingFace
      addLog('1. Opening https://huggingface.co/models...');
      const sessionResult = await createBrowserSession({
        url: 'https://huggingface.co/models',
      });
      const newSessionId = sessionResult.session_id;
      setSessionId(newSessionId);
      addLog(`Session created: ${newSessionId}`);

      // 2. Input "gpt2" into search
      addLog('2. Inputting "gpt2" into search box...');
      // HuggingFace search input usually has placeholder "Search models, datasets, users..."
      // We'll try a generic selector that usually works there
      const inputResult = await inputTextTool.execute(
        {
          sessionId: newSessionId,
          selector: 'input[placeholder*="Search"]',
          text: 'gpt2',
        },
        executeScript,
      );
      addLog(`Input Result: ${JSON.stringify(inputResult)}`);

      // 3. Wait a bit for SPA update (simulating user pause)
      addLog('3. Waiting 2s for SPA update...');
      await new Promise((resolve) => setTimeout(resolve, 2000));

      // 4. Extract Content to see if results updated
      addLog('4. Extracting content...');
      const extractResult = await extractWebContentTool.execute(
        { sessionId: newSessionId, saveRawHtml: false },
        executeScript,
      );
      const contentItem = extractResult.result?.content?.[0];
      const content = contentItem?.type === 'text' ? contentItem.text : '';

      if (content.includes('openai-community/gpt2')) {
        addLog('✅ SUCCESS: Found "gpt2" results in content.');
      } else {
        addLog(
          '⚠️ WARNING: Did not find expected "gpt2" results. SPA update might be slow or selector failed.',
        );
      }
    } catch (error) {
      addLog(`HuggingFace test failed: ${error}`);
    } finally {
      setIsRunning(false);
      addLog('HuggingFace test finished');
    }
  };

  return (
    <Card className="w-full max-w-2xl mx-auto my-4">
      <CardHeader>
        <CardTitle>Browser Tool Deadlock Tester</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <div className="grid w-full items-center gap-1.5">
            <Label htmlFor="url">Target URL</Label>
            <Input
              id="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com"
            />
          </div>
        </div>

        <div className="flex gap-2 flex-wrap">
          <Button
            onClick={handleCreateSession}
            disabled={isRunning || !!sessionId}
          >
            Create Session
          </Button>
          <Button
            onClick={handleCloseSession}
            disabled={!sessionId}
            variant="destructive"
          >
            Close Session
          </Button>
          <Button
            onClick={runStressTest}
            disabled={isRunning || !sessionId}
            variant="secondary"
          >
            Run Tool Test
          </Button>
          <Button
            onClick={runWikipediaTest}
            disabled={isRunning}
            variant="outline"
          >
            Test Wikipedia
          </Button>
          <Button
            onClick={runHuggingFaceTest}
            disabled={isRunning}
            variant="outline"
          >
            Test HuggingFace
          </Button>
        </div>

        <div className="mt-4">
          <Label>Logs</Label>
          <div className="h-[300px] w-full rounded-md border p-4 bg-slate-950 text-slate-50 font-mono text-xs overflow-y-auto">
            {logs.map((log, i) => (
              <div key={i} className="mb-1">
                {log}
              </div>
            ))}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
