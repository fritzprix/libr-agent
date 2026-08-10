import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { ToolStructuredResult } from '../ToolStructuredResult';

const navigateMock = vi.fn();

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>(
    'react-router-dom',
  );
  return {
    ...actual,
    useNavigate: () => navigateMock,
  };
});

vi.mock('@/lib/backend', () => ({
  openPathWithDefaultApp: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

function renderAgentCard(
  toolName: string,
  data: Record<string, unknown>,
  toolArgs?: Record<string, unknown>,
) {
  return render(
    <MemoryRouter>
      <ToolStructuredResult
        toolName={toolName}
        data={data}
        toolArgs={toolArgs}
      />
    </MemoryRouter>,
  );
}

describe('ToolStructuredResult agent session cards', () => {
  it('renders spawn card with assistant name, workspace override, and collapsed mission', () => {
    const longMission = Array.from(
      { length: 6 },
      (_, i) => `Mission line ${i + 1}`,
    ).join('\n');

    renderAgentCard(
      'agent__startSession',
      {
        sessionId: 'a1b2c3d4e5',
        status: 'started',
        responseStatus: 'pending',
        assistantName: 'Writer Agent',
        workspaceOverride: true,
        workspacePath: 'C:/work/shared',
        task: longMission,
      },
    );

    const card = screen.getByTestId('tool-structured-agent-session');
    expect(card).toHaveAttribute('data-card-kind', 'spawned');
    expect(screen.getByText('Writer Agent')).toBeInTheDocument();
    expect(screen.queryByText('a1b2c3d4e5')).not.toBeInTheDocument();
    expect(
      screen.getByTestId('agent-session-workspace-override'),
    ).toHaveTextContent(/Workspace override/i);
    expect(
      screen.getByTestId('agent-session-instruction-text'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /show more/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /open session/i }),
    ).toBeInTheDocument();
  });

  it('shows isolated workspace when override was not set', () => {
    renderAgentCard('agent__startSession', {
      sessionId: 'a1b2c3d4e5',
      status: 'started',
      responseStatus: 'pending',
      assistantName: 'Researcher',
      workspaceOverride: false,
      task: 'Quick task',
    });

    expect(
      screen.getByTestId('agent-session-workspace-default'),
    ).toHaveTextContent(/Isolated workspace/i);
  });

  it('renders harvest finished card with expandable result', () => {
    const longResult = Array.from({ length: 8 }, (_, i) => `Line ${i + 1}`).join(
      '\n',
    );

    renderAgentCard('agent__checkSession', {
      sessionId: 'a1b2c3d4e5',
      status: 'idle',
      responseStatus: 'success',
      turnCount: 3,
      assistantName: 'Writer',
      result: longResult,
    });

    expect(screen.getByTestId('tool-structured-agent-session')).toHaveAttribute(
      'data-card-kind',
      'finished',
    );
    expect(screen.getByTestId('agent-session-result-text')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /show result/i }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /show result/i }));
    expect(
      screen.getByRole('button', { name: /hide result/i }),
    ).toBeInTheDocument();
  });

  it('renders wait timeout without agent wait CTAs', () => {
    renderAgentCard('agent__checkSession', {
      sessionId: 'a1b2c3d4e5',
      status: 'busy',
      responseStatus: 'timeout',
      timeout: true,
      timeoutSeconds: 600,
      latestMessages: [{ role: 'assistant', summary: 'Drafting section 3' }],
    });

    const card = screen.getByTestId('tool-structured-agent-session');
    expect(card).toHaveAttribute('data-card-kind', 'wait_timeout');
    expect(screen.getByText(/stopped waiting/i)).toBeInTheDocument();
    expect(screen.getByText(/Drafting section 3/i)).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /wait again/i }),
    ).not.toBeInTheDocument();
  });

  it('renders message card with assistant name, not session id or jargon', () => {
    renderAgentCard(
      'agent__messageToSession',
      {
        sessionId: '01c8d89ccb',
        messageId: 'msg-1',
        status: 'processed',
        responseStatus: 'pending',
        assistantName: 'Research Buddy',
        instruction: 'Please add margins to the draft',
      },
    );

    const card = screen.getByTestId('tool-structured-agent-session');
    expect(card).toHaveAttribute('data-card-kind', 'instruction_sent');
    expect(screen.getByText('Messaged')).toBeInTheDocument();
    expect(screen.getByText('Research Buddy')).toBeInTheDocument();
    expect(screen.queryByText('01c8d89ccb')).not.toBeInTheDocument();
    expect(screen.queryByText(/instruction sent/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/processed/i)).not.toBeInTheDocument();
    expect(screen.getByText('Message')).toBeInTheDocument();
    expect(
      screen.getByTestId('agent-session-instruction-text'),
    ).toHaveTextContent(/Please add margins/i);
  });

  it('navigates to child session on open', () => {
    navigateMock.mockClear();

    renderAgentCard(
      'agent__messageToSession',
      {
        sessionId: 'a1b2c3d4e5',
        messageId: 'msg-1',
        status: 'accepted',
        responseStatus: 'pending',
        assistantName: 'Helper',
      },
      { message: 'Please add margins' },
    );

    fireEvent.click(
      screen.getByRole('button', { name: /open session/i }),
    );
    expect(navigateMock).toHaveBeenCalledWith('/agent/a1b2c3d4e5');
  });

  it('hides open for deleted sessions', () => {
    renderAgentCard('agent__deleteSession', {
      sessionId: 'a1b2c3d4e5',
      deleted: true,
      descendantCount: 1,
      responseStatus: 'success',
    });

    expect(screen.getByTestId('tool-structured-agent-session')).toHaveAttribute(
      'data-card-kind',
      'deleted',
    );
    expect(
      screen.queryByRole('button', { name: /open session/i }),
    ).not.toBeInTheDocument();
  });
});
