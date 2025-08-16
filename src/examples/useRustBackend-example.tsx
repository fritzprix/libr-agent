import { useState } from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

/**
 * Example component demonstrating type-safe Rust backend usage
 */
export function RustBackendExample() {
  const [fileName, setFileName] = useState('');
  const [fileContent, setFileContent] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>('');

  const rustBackend = useRustBackend();

  const handleReadFile = async () => {
    if (!fileName.trim()) {
      setError('Please enter a file name');
      return;
    }

    setLoading(true);
    setError('');

    try {
      // Type-safe file reading with automatic error handling and logging
      const fileData = await rustBackend.readFile(fileName);

      // Convert to text (assuming it's a text file)
      const text = new TextDecoder().decode(new Uint8Array(fileData));
      setFileContent(text);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to read file');
    } finally {
      setLoading(false);
    }
  };

  const handleGreeting = async () => {
    try {
      const greeting = await rustBackend.greet('TypeScript User');
      alert(greeting);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to get greeting');
    }
  };

  const handleGetLogsDir = async () => {
    try {
      const logsDir = await rustBackend.getAppLogsDir();
      alert(`Logs directory: ${logsDir}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to get logs dir');
    }
  };

  return (
    <div className="p-4 space-y-4">
      <h2 className="text-xl font-bold">Rust Backend Example</h2>

      {error && <div className="text-red-500 text-sm">{error}</div>}

      <div className="space-y-2">
        <h3 className="font-semibold">File Reading</h3>
        <div className="flex gap-2">
          <Input
            value={fileName}
            onChange={(e) => setFileName(e.target.value)}
            placeholder="Enter file path"
            className="flex-1"
          />
          <Button onClick={handleReadFile} disabled={loading}>
            {loading ? 'Reading...' : 'Read File'}
          </Button>
        </div>

        {fileContent && (
          <textarea
            value={fileContent}
            readOnly
            className="w-full h-32 p-2 border rounded resize-none"
            placeholder="File content will appear here..."
          />
        )}
      </div>

      <div className="space-y-2">
        <h3 className="font-semibold">Other Functions</h3>
        <div className="flex gap-2">
          <Button onClick={handleGreeting}>Test Greeting</Button>
          <Button onClick={handleGetLogsDir}>Get Logs Directory</Button>
        </div>
      </div>

      <div className="text-sm text-gray-600">
        <p>
          <strong>Benefits of useRustBackend:</strong>
        </p>
        <ul className="list-disc list-inside space-y-1">
          <li>✅ Full TypeScript type safety</li>
          <li>✅ Automatic error handling and logging</li>
          <li>✅ Centralized Rust command management</li>
          <li>✅ IntelliSense support</li>
          <li>✅ Consistent API interface</li>
        </ul>
      </div>
    </div>
  );
}

export default RustBackendExample;
