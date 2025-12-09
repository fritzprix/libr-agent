import { describe, it, expect, vi } from 'vitest';
import { extractWebContentTool } from './ExtractContentTool';
import { readWebContentTool } from './ReadContentTool';
import { ContentStore } from './content-store';

describe('Empty Content Handling', () => {
    const sessionId = 'test-empty-session';

    it('ExtractContentTool should warn with correct tool suggestion', async () => {
        const executeScript = vi.fn().mockResolvedValue('<body></body>');

        const result = await extractWebContentTool.execute({ sessionId }, executeScript);

        const resultString = JSON.stringify(result);
        expect(resultString).toContain('(Empty Page) The extracted content is empty');
        expect(resultString).toContain("extractWebContent");
        expect(resultString).toContain("saveRawHtml");
    });

    it('ReadContentTool should warn with correct tool suggestion', async () => {
        ContentStore.saveContent(sessionId, ''); // Empty content

        const result = await readWebContentTool.execute({ sessionId, page: 1 });
        const resultString = JSON.stringify(result);

        expect(resultString).toContain('(Empty Page) The extracted content is empty');
        expect(resultString).toContain("extractWebContent");
    });
});
