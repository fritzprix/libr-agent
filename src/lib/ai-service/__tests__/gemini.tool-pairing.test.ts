import { describe, it, expect, vi } from 'vitest';
import { convertToGeminiMessages } from '../gemini/mapper';
import { Message } from '@/models/chat';

// Mock the Google AI SDK
vi.mock('@google/genai', () => ({
    createPartFromFunctionResponse: vi.fn((id, name, response) => ({
        functionResponse: { id, name, response },
    })),
    HarmCategory: {},
    HarmBlockThreshold: {},
}));

// Mock the logger
vi.mock('../../logger', () => ({
    getLogger: () => ({
        debug: vi.fn(),
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
    }),
}));

describe('GeminiService Tool Result Handling', () => {

    it('should correctly convert tool result to FunctionResponse part using history', () => {
        const toolCallId = 'call_123';
        const toolName = 'get_weather';

        const messages: Message[] = [
            {
                id: '1',
                sessionId: 's1',
                threadId: 's1',
                role: 'user',
                content: [{ type: 'text', text: 'What is the weather?' }],
            },
            {
                id: '2',
                sessionId: 's1',
                threadId: 's1',
                role: 'assistant',
                content: [],
                tool_calls: [
                    {
                        id: toolCallId,
                        type: 'function',
                        function: {
                            name: toolName,
                            arguments: JSON.stringify({ location: 'Seoul' }),
                        },
                    },
                ],
            },
            {
                id: '3',
                sessionId: 's1',
                threadId: 's1',
                role: 'tool',
                tool_call_id: toolCallId,
                content: [{ type: 'text', text: JSON.stringify({ temp: 25 }) }],
            },
        ];

        const result = convertToGeminiMessages(messages);

        // Expecting 3 messages
        // 1. user: What is the weather?
        // 2. assistant: Tool call
        // 3. tool result -> user: FunctionResponse

        expect(result.length).toBe(3);
        const m0 = result[0];
        const m1 = result[1];
        const m2 = result[2];

        expect(m0).toBeDefined();
        expect(m1).toBeDefined();
        expect(m2).toBeDefined();

        if (!m0 || !m1 || !m2 || !m2.parts) return; // For TS safety

        expect(m0.role).toBe('user');
        expect(m1.role).toBe('model');
        expect(m2.role).toBe('user');

        const firstPart = m2.parts[0];
        expect(firstPart).toBeDefined();
        if (!firstPart) return;

        expect(firstPart).toEqual({
            functionResponse: {
                id: toolCallId,
                name: toolName,
                response: { temp: 25 },
            },
        });
    });

    it('should fallback to text part if tool name is not found in history', () => {
        const messages: Message[] = [
            {
                id: '3',
                sessionId: 's1',
                threadId: 's1',
                role: 'tool',
                tool_call_id: 'unknown_call',
                content: [{ type: 'text', text: 'Result content' }],
            },
        ];

        const result = convertToGeminiMessages(messages);

        expect(result.length).toBe(1);
        const firstMsg = result[0];
        expect(firstMsg).toBeDefined();
        if (!firstMsg || !firstMsg.parts) return;

        const firstPart = firstMsg.parts[0];
        expect(firstPart).toBeDefined();
        if (!firstPart) return;

        expect(firstPart).toHaveProperty('text', 'Result content');
    });

    it('should prepend synthetic user message when first message is model (e.g. playbook start on fresh session)', () => {
        const toolCallId = 'call_playbook';

        // Simulates: start button injects [assistantToolCall, toolResult] with no prior user message
        const messages: Message[] = [
            {
                id: '1',
                sessionId: 's1',
                threadId: 's1',
                role: 'assistant',
                content: [],
                tool_calls: [
                    {
                        id: toolCallId,
                        type: 'function',
                        function: {
                            name: 'playbook__selectPlaybook',
                            arguments: JSON.stringify({ id: 'pb_123' }),
                        },
                    },
                ],
            },
            {
                id: '2',
                sessionId: 's1',
                threadId: 's1',
                role: 'tool',
                tool_call_id: toolCallId,
                content: [{ type: 'text', text: 'Playbook loaded.' }],
            },
        ];

        const result = convertToGeminiMessages(messages);

        // Must start with user, then model (tool call), then user (tool result)
        expect(result.length).toBe(3);
        expect(result[0]?.role).toBe('user');
        expect(result[1]?.role).toBe('model');
        expect(result[2]?.role).toBe('user');
    });

    it('should attach dummy thought signature to first functionCall when missing', () => {
        const messages: Message[] = [
            {
                id: '1',
                sessionId: 's1',
                threadId: 's1',
                role: 'user',
                content: [{ type: 'text', text: 'Run a tool' }],
            },
            {
                id: '2',
                sessionId: 's1',
                threadId: 's1',
                role: 'assistant',
                content: [],
                tool_calls: [
                    {
                        id: 'call_1',
                        type: 'function',
                        function: {
                            name: 'workspace__executePendingShell',
                            arguments: JSON.stringify({ executionId: 'abc' }),
                        },
                    },
                ],
            },
        ];

        const result = convertToGeminiMessages(messages);

        expect(result.length).toBe(2);
        const modelMessage = result[1];

        expect(modelMessage).toBeDefined();
        expect(modelMessage?.role).toBe('model');
        expect(modelMessage?.parts?.length).toBe(1);

        const firstPart = modelMessage?.parts?.[0] as {
            functionCall?: { name?: string };
            thoughtSignature?: string;
        };

        expect(firstPart.functionCall?.name).toBe(
            'workspace__executePendingShell',
        );
        expect(firstPart.thoughtSignature).toBe(
            'skip_thought_signature_validator',
        );
    });
});
