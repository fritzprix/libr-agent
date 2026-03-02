import { render } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentToolCallDetails } from '../AgentToolCallDetails';
import type { ToolCall, Message } from '@/models/chat';
import * as toolCallUtils from '@/lib/tool-call-utils';

// Mock dependencies that AgentToolCallDetails pulls in via AgentMessageRenderer
vi.mock('@/hooks/use-rust-backend', () => ({
    useRustBackend: () => ({ openExternalUrl: vi.fn() }),
}));

vi.mock('@/hooks/use-settings', () => ({
    useSettings: () => ({
        value: { toolCallGroupVisibleCount: 4, display: { toolDetailLevel: 'developer' } },
        update: vi.fn(),
        isLoading: false,
        error: null,
    }),
}));

vi.mock('@/context/AgentChatContext', () => ({
    useAgentChatActions: () => ({ submit: vi.fn(), injectMessages: vi.fn() }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
    useAgentSessionState: () => ({
        session: { id: 'test-session', assistant: { id: 'test-assistant' } },
    }),
}));

vi.mock('next-themes', () => ({
    useTheme: () => ({ resolvedTheme: 'light' }),
}));

vi.mock('@/hooks/useClipboard', () => ({
    useClipboard: () => ({ copied: false, copyToClipboard: vi.fn() }),
}));

vi.mock('@/lib/logger', () => ({
    getLogger: () => ({
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
        debug: vi.fn(),
    }),
}));

// Minimal fixtures
const makeToolCall = (args = '{}'): ToolCall => ({
    id: 'call-1',
    type: 'function',
    function: { name: 'test__myTool', arguments: args },
});

const makeToolResult = (text: string): Message =>
    ({
        id: 'msg-result-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        content: [{ type: 'text', text }],
        tool_call_id: 'call-1',
    }) as Message;

describe('AgentToolCallDetails', () => {
    it('renders nothing when showDetails is false', () => {
        const { container } = render(
            <AgentToolCallDetails
                toolCall={makeToolCall()}
                showDetails={false}
            />,
        );
        // Component must return null — no DOM output at all
        expect(container.firstChild).toBeNull();
    });

    it('renders parameters section when showDetails is true and args are provided', () => {
        const { getByText } = render(
            <AgentToolCallDetails
                toolCall={makeToolCall('{"path": "src/index.ts"}')}
                showDetails={true}
            />,
        );
        expect(getByText('Parameters')).toBeInTheDocument();
    });

    it('renders result section when showDetails is true and toolResult is provided', () => {
        const { getByText } = render(
            <AgentToolCallDetails
                toolCall={makeToolCall()}
                toolResult={makeToolResult('File contents here')}
                showDetails={true}
            />,
        );
        expect(getByText('Result')).toBeInTheDocument();
    });

    it('renders error section with "Error Details" label when hasError is true', () => {
        const { getByText } = render(
            <AgentToolCallDetails
                toolCall={makeToolCall()}
                toolResult={makeToolResult('Something failed')}
                hasError={true}
                showDetails={true}
            />,
        );
        expect(getByText('Error Details')).toBeInTheDocument();
    });

    it('renders loading indicator when isLoading is true', () => {
        const { getByText } = render(
            <AgentToolCallDetails
                toolCall={makeToolCall()}
                isLoading={true}
                showDetails={true}
            />,
        );
        expect(getByText('Executing tool...')).toBeInTheDocument();
    });

    it('uses parsedArgs directly and skips parseToolArguments when parsedArgs is provided', () => {
        const parseSpy = vi.spyOn(toolCallUtils, 'parseToolArguments');
        const preParseResult = { file: 'src/main.ts', line: 42 };

        const { getByText } = render(
            <AgentToolCallDetails
                toolCall={makeToolCall('{"should":"not-be-parsed"}')}
                parsedArgs={preParseResult}
                showDetails={true}
            />,
        );

        // parsedArgs data must be displayed (pre-parsed object used, not the raw argument string)
        expect(getByText(/src\/main\.ts/)).toBeInTheDocument();
        // parseToolArguments must NOT have been called — no redundant JSON.parse
        expect(parseSpy).not.toHaveBeenCalled();

        parseSpy.mockRestore();
    });
});
