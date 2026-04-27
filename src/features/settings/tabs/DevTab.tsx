import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import type { ServiceConfig } from '@/lib/services/settings-service';
import type { Message } from '@/models/chat';
import { Button, Textarea } from '@/components/ui';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { getLogger } from '@/lib/logger';

const logger = getLogger('DevTab');

const TEST_SESSION_ID = 'dev-test-session';

const SAMPLE_MESSAGES: Message[] = [
  {
    id: '1',
    sessionId: TEST_SESSION_ID,
    threadId: TEST_SESSION_ID,
    role: 'user',
    content: [
      { type: 'text', text: 'Tell me about the theory of relativity.' },
    ],
  },
  {
    id: '2',
    sessionId: TEST_SESSION_ID,
    threadId: TEST_SESSION_ID,
    role: 'assistant',
    content: [
      {
        type: 'text',
        text: 'The theory of relativity consists of two interrelated physics theories: special relativity and general relativity. Special relativity, proposed by Albert Einstein in 1905, establishes that the laws of physics are the same for all non-accelerating observers and that the speed of light in a vacuum is the same regardless of the motion of the light source. A key consequence is E=mc², showing mass-energy equivalence. General relativity, published in 1915, extends this to include gravity, describing it as a curvature of spacetime caused by mass and energy.',
      },
    ],
  },
  {
    id: '3',
    sessionId: TEST_SESSION_ID,
    threadId: TEST_SESSION_ID,
    role: 'user',
    content: [{ type: 'text', text: 'How does time dilation work?' }],
  },
  {
    id: '4',
    sessionId: TEST_SESSION_ID,
    threadId: TEST_SESSION_ID,
    role: 'assistant',
    content: [
      {
        type: 'text',
        text: 'Time dilation is a difference in elapsed time measured by two clocks that are either moving relative to each other or are at different gravitational potentials. Under special relativity, a moving clock ticks slower relative to a stationary observer — this is velocity-based time dilation. Under general relativity, clocks in stronger gravitational fields tick slower than those in weaker fields. Both effects are measurable and have practical implications, such as GPS satellites requiring clock corrections.',
      },
    ],
  },
];

interface TestResult {
  type: 'sampleText' | 'compact';
  output: string;
  model?: string;
  usage?: {
    promptTokens?: number;
    completionTokens?: number;
    totalTokens?: number;
  };
  durationMs: number;
  error?: string;
}

interface DevTabProps {
  serviceConfigs: Record<string, ServiceConfig>;
}

export default function DevTab({ serviceConfigs }: DevTabProps) {
  const { t } = useTranslation('common');
  const [selectedProvider, setSelectedProvider] = useState<AIServiceProvider>(
    AIServiceProvider.OpenAI,
  );
  const [prompt, setPrompt] = useState(
    'Summarize the key points of quantum mechanics in 3 sentences.',
  );
  const [result, setResult] = useState<TestResult | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  const getService = useCallback(() => {
    const config = serviceConfigs[selectedProvider];
    const apiKey = config?.apiKey || '';
    return AIServiceFactory.getService(selectedProvider, apiKey);
  }, [selectedProvider, serviceConfigs]);

  const runSampleText = useCallback(async () => {
    setIsRunning(true);
    setResult(null);
    const start = Date.now();
    try {
      const service = getService();
      const response = await service.sampleText(prompt);
      const text =
        response.result?.content
          ?.filter((c) => c.type === 'text')
          .map((c) => (c as { type: 'text'; text: string }).text)
          .join('') || t('settings.dev.emptyResponse', '(empty response)');
      setResult({
        type: 'sampleText',
        output: text,
        model: response.result?.sampling?.model,
        usage: response.result?.sampling?.usage,
        durationMs: Date.now() - start,
      });
    } catch (error: unknown) {
      const msg = error instanceof Error ? error.message : String(error);
      logger.error('sampleText test failed', error);
      setResult({
        type: 'sampleText',
        output: '',
        durationMs: Date.now() - start,
        error: msg,
      });
    } finally {
      setIsRunning(false);
    }
  }, [getService, prompt]);

  const runCompact = useCallback(async () => {
    setIsRunning(true);
    setResult(null);
    const start = Date.now();
    try {
      const service = getService();
      const summary = await service.compact(SAMPLE_MESSAGES);
      setResult({
        type: 'compact',
        output: summary,
        durationMs: Date.now() - start,
      });
    } catch (error: unknown) {
      const msg = error instanceof Error ? error.message : String(error);
      logger.error('compact test failed', error);
      setResult({
        type: 'compact',
        output: '',
        durationMs: Date.now() - start,
        error: msg,
      });
    } finally {
      setIsRunning(false);
    }
  }, [getService]);

  const providers = Object.values(AIServiceProvider).filter(
    (p) => p !== AIServiceProvider.Empty,
  );

  return (
    <div className="space-y-6 p-1">
      <div className="flex items-center gap-2">
        <span className="rounded bg-yellow-500/20 px-2 py-0.5 text-xs font-mono text-yellow-500">
          {t('settings.dev.devOnly', 'DEV ONLY')}
        </span>
        <h2 className="text-sm font-semibold">{t('settings.dev.testerTitle', 'sampleText / compact tester')}</h2>
      </div>

      {/* Provider selector */}
      <div className="space-y-1">
        <label className="text-xs font-medium text-muted-foreground">
          {t('settings.dev.provider', 'Provider')}
        </label>
        <Select
          value={selectedProvider}
          onValueChange={(v) => setSelectedProvider(v as AIServiceProvider)}
        >
          <SelectTrigger className="w-48">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {providers.map((p) => (
              <SelectItem key={p} value={p}>
                {p}
                {serviceConfigs[p]?.apiKey ? ' ✓' : t('settings.dev.noKey', ' (no key)')}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Prompt input */}
      <div className="space-y-1">
        <label className="text-xs font-medium text-muted-foreground">
          {t('settings.dev.promptLabel', 'Prompt (for sampleText)')}
        </label>
        <Textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          rows={3}
          className="font-mono text-xs"
          placeholder={t('settings.dev.promptPlaceholder', 'Enter a test prompt...')}
        />
      </div>

      {/* Compact preview */}
      <div className="rounded border border-dashed border-border p-3 text-xs text-muted-foreground">
        <p className="mb-1 font-medium">
          {t('settings.dev.compactUses', 'compact() uses {{count}} sample messages:', { count: SAMPLE_MESSAGES.length })}
        </p>
        {SAMPLE_MESSAGES.map((m) => {
          const text = m.content
            .filter((c) => c.type === 'text')
            .map((c) => (c as { type: 'text'; text: string }).text)
            .join('');
          return (
            <p key={m.id} className="truncate">
              <span className="font-mono text-foreground/60">[{m.role}]</span>{' '}
              {text.slice(0, 80)}
              {text.length > 80 ? '…' : ''}
            </p>
          );
        })}
      </div>

      {/* Action buttons */}
      <div className="flex gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={runSampleText}
          disabled={isRunning || !prompt.trim()}
        >
          {isRunning ? t('settings.dev.running', 'Running…') : t('settings.dev.runSampleText', 'Run sampleText()')}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={runCompact}
          disabled={isRunning}
        >
          {isRunning ? t('settings.dev.running', 'Running…') : t('settings.dev.runCompact', 'Run compact()')}
        </Button>
      </div>

      {/* Result display */}
      {result && (
        <div
          className={`rounded border p-3 text-xs ${result.error ? 'border-destructive/50 bg-destructive/5' : 'border-border bg-muted/30'}`}
        >
          <div className="mb-2 flex items-center gap-3 font-mono">
            <span className="font-semibold">{result.type}()</span>
            {result.error ? (
              <span className="text-destructive">{t('settings.dev.error', 'ERROR')}</span>
            ) : (
              <span className="text-green-500">{t('settings.dev.ok', 'OK')}</span>
            )}
            <span className="text-muted-foreground">{result.durationMs}ms</span>
            {result.model && (
              <span className="text-muted-foreground">
                {t('settings.dev.model', 'model: {{model}}', { model: result.model })}
              </span>
            )}
          </div>
          {result.usage && (
            <p className="mb-2 text-muted-foreground">
              {t('settings.dev.tokensInfo', 'tokens: {{promptTokens}} prompt + {{completionTokens}} completion = {{totalTokens}} total', {
                promptTokens: result.usage.promptTokens ?? '?',
                completionTokens: result.usage.completionTokens ?? '?',
                totalTokens: result.usage.totalTokens ?? '?'
              })}
            </p>
          )}
          {result.error ? (
            <pre className="whitespace-pre-wrap text-destructive">
              {result.error}
            </pre>
          ) : (
            <pre className="max-h-60 overflow-auto whitespace-pre-wrap">
              {result.output}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
