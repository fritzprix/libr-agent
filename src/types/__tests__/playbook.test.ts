import { describe, it, expect } from 'vitest';
import type { Playbook } from '../playbook';

describe('Playbook Type Definitions', () => {
    it('should accept playbook with inputs', () => {
        const playbook: Playbook = {
            title: 'Company Analysis',
            goal: 'Analyze {{targetCompany}} financials',
            inputs: [
                {
                    name: 'targetCompany',
                    description: 'Company to analyze',
                    type: 'string',
                },
            ],
            workflow: [],
            successCriteria: {
                description: 'Financial report generated',
            },
        };

        expect(playbook.inputs).toHaveLength(1);
        expect(playbook.title).toBe('Company Analysis');
    });

    it('should accept playbook without inputs for backward compatibility', () => {
        const playbook: Playbook = {
            title: 'Simple Task',
            goal: 'Do something',
            inputs: [],
            workflow: [],
            successCriteria: {
                description: 'Task completed',
            },
            agentId: 'agent-123',  // optional field
            initialCommand: 'do it',
        };

        expect(playbook.inputs).toHaveLength(0);
        expect(playbook.agentId).toBe('agent-123');
    });
});
