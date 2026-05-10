import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';

import { PendingApprovalWidget } from '../PendingApprovalWidget';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue ?? key,
  }),
}));

describe('PendingApprovalWidget', () => {
  it('surfaces hard approvals distinctly when YOLO mode is enabled', () => {
    render(
      <PendingApprovalWidget
        approvals={[
          {
            toolCallId: 'call-hard',
            toolName: 'runShell',
            arguments: '{"command":"rm -rf /important"}',
            approvalKind: 'hard',
            requestId: 'req-hard',
            description: 'Hard approval',
            inputPreview: 'rm -rf /important',
          },
        ]}
        yoloModeEnabled
        onRespond={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText('Hard Approval Required')).toBeInTheDocument();
    expect(screen.getByText('Hard approval')).toBeInTheDocument();
    expect(
      screen.getByText(
        'YOLO mode is on, but this high-risk action still requires manual approval.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Approve high-risk action' }),
    ).toBeInTheDocument();
  });
});
