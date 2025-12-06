import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { LocalDatabase } from '../database';
import { playbooksCRUD } from '../crud';
import type { Playbook } from '@/types/playbook';

describe('Playbook CRUD with new schema', () => {
    beforeEach(async () => {
        LocalDatabase.resetInstance();
        await LocalDatabase.getInstance().open();
    });

    afterEach(async () => {
        await LocalDatabase.getInstance().delete();
        LocalDatabase.resetInstance();
    });

    it('should create playbook with inputs and title', async () => {
        const playbook: Playbook & { id: string } = {
            id: 'test-id',
            title: 'Test Playbook',
            goal: 'Test {{target}}',
            inputs: [
                { name: 'target', description: 'Test target', type: 'string' },
            ],
            workflow: [],
            successCriteria: { description: 'Done' },
        };

        await playbooksCRUD.upsert(playbook);
        const records = await playbooksCRUD.getPage(1, 10);

        expect(records.items).toHaveLength(1);
        expect(records.items[0].title).toBe('Test Playbook');
        expect(records.items[0].inputs).toHaveLength(1);
        expect(records.items[0].inputs[0].name).toBe('target');
    });

    it('should support pagination by agentId', async () => {
        const agent1 = 'agent-1';
        const agent2 = 'agent-2';

        const pb1: Playbook & { id: string } = {
            id: 'pb-1',
            title: 'PB1',
            goal: 'G1',
            inputs: [],
            workflow: [],
            successCriteria: { description: 'D1' },
            agentId: agent1,
        };

        const pb2: Playbook & { id: string } = {
            id: 'pb-2',
            title: 'PB2',
            goal: 'G2',
            inputs: [],
            workflow: [],
            successCriteria: { description: 'D2' },
            agentId: agent2,
        };

        await playbooksCRUD.upsert(pb1);
        await playbooksCRUD.upsert(pb2);

        const page1 = await playbooksCRUD.getPageForAgent(agent1, 1, 10);
        expect(page1.items).toHaveLength(1);
        expect(page1.items[0].title).toBe('PB1');

        const page2 = await playbooksCRUD.getPageForAgent(agent2, 1, 10);
        expect(page2.items).toHaveLength(1);
        expect(page2.items[0].title).toBe('PB2');
    });
});
