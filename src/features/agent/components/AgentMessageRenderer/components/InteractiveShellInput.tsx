import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Loader2, XCircle } from 'lucide-react';
import { UIActionResult } from '@mcp-ui/client';

export interface InteractiveShellInputProps {
  resource: {
    uri: string;
    mimeType: string;
    text?: string;
  };
  onUIAction: (action: UIActionResult) => void;
}

export const InteractiveShellInput: React.FC<InteractiveShellInputProps> = ({
  resource,
  onUIAction,
}) => {
  const { t } = useTranslation('common');
  const [input, setInput] = useState('');
  const [status, setStatus] = useState<'idle' | 'submitting' | 'cancelled'>(
    'idle',
  );

  let parsed: any;
  try {
    parsed = JSON.parse(resource.text || '{}');
  } catch (e) {
    return (
      <div className="text-destructive text-sm p-4 border rounded">
        Failed to parse shell input parameters.
      </div>
    );
  }

  const { execution_id, prompt, input_type, nonce } = parsed;

  const obfuscate = (text: string, nonceStr: string) => {
    const textEncoder = new TextEncoder();
    const inputBytes = textEncoder.encode(text);
    const nonceBytes = textEncoder.encode(nonceStr);
    const xored = new Uint8Array(inputBytes.length);
    for (let i = 0; i < inputBytes.length; i++) {
      xored[i] = inputBytes[i] ^ nonceBytes[i % nonceBytes.length];
    }
    // Base64 encoding
    let binary = '';
    for (let i = 0; i < xored.length; i++) {
      binary += String.fromCharCode(xored[i]);
    }
    return btoa(binary);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input || status !== 'idle') return;

    setStatus('submitting');
    const obfuscatedInput = obfuscate(input, nonce);

    onUIAction({
      type: 'tool',
      payload: {
        toolName: 'workspace__executePendingShell',
        params: {
          executionId: execution_id,
          userInput: obfuscatedInput,
        },
      },
    });

    setInput('');
  };

  const handleCancel = () => {
    if (status !== 'idle') return;
    setStatus('cancelled');

    onUIAction({
      type: 'tool',
      payload: {
        toolName: 'workspace__cancelPendingExecution',
        params: {
          executionId: execution_id,
        },
      },
    });
  };

  if (status === 'submitting') {
    return (
      <div className="flex flex-col items-center justify-center p-6 bg-card border rounded-lg text-muted-foreground">
        <Loader2 className="w-5 h-5 animate-spin mb-2" />
        <span className="text-sm">Executing command...</span>
      </div>
    );
  }

  if (status === 'cancelled') {
    return (
      <div className="flex flex-col items-center justify-center p-6 bg-card border rounded-lg text-muted-foreground">
        <XCircle className="w-5 h-5 mb-2 text-destructive" />
        <span className="text-sm">Cancelled</span>
      </div>
    );
  }

  return (
    <div className="p-4 bg-card border rounded-lg max-w-lg">
      <h3 className="text-sm font-medium mb-3 text-foreground">{prompt}</h3>
      <form onSubmit={handleSubmit} className="flex flex-col gap-3">
        <Input
          type={input_type === 'password' ? 'password' : 'text'}
          placeholder="Enter input..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          required
          autoFocus
          className="w-full bg-background"
        />
        <div className="flex items-center gap-2">
          <Button type="submit" size="sm" disabled={!input}>
            Submit
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={handleCancel}
          >
            Cancel
          </Button>
        </div>
      </form>
    </div>
  );
};
