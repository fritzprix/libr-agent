import { describe, it, expect } from 'vitest';
import {
  PlaybookStepSchema,
  PlaybookWorkflowSchema,
  SuccessCriteriaSchema,
  parsePlaybookWorkflow,
  parseSuccessCriteria,
  safeParsePlaybookWorkflow,
  safeParseSuccessCriteria,
  interpolateVariables,
} from '../playbook';

describe('Playbook Schemas Validation', () => {
  describe('PlaybookStepSchema', () => {
    it('validates a correct step', () => {
      const step = {
        stepId: '1',
        description: 'Test step',
        action: {
          toolName: 'test-tool',
          purpose: 'Testing',
        },
        requiredData: ['input'],
        outputVariable: 'result',
      };
      const result = PlaybookStepSchema.safeParse(step);
      expect(result.success).toBe(true);
    });

    it('fails if required fields are missing', () => {
      const result = PlaybookStepSchema.safeParse({ stepId: '1' });
      expect(result.success).toBe(false);
    });
  });

  describe('PlaybookWorkflowSchema', () => {
    it('validates a correct workflow', () => {
      const workflow = {
        steps: [
          {
            stepId: '1',
            description: 'Test step',
            action: {
              toolName: 'test-tool',
              purpose: 'Testing',
            },
            requiredData: [],
            outputVariable: 'result',
          },
        ],
        metadata: { key: 'value' },
        version: '1.0',
      };
      const result = PlaybookWorkflowSchema.safeParse(workflow);
      expect(result.success).toBe(true);
    });

    it('validates a workflow without optional fields', () => {
      const workflow = {
        steps: [],
      };
      const result = PlaybookWorkflowSchema.safeParse(workflow);
      expect(result.success).toBe(true);
    });

    it('fails if steps are not provided', () => {
      const result = PlaybookWorkflowSchema.safeParse({});
      expect(result.success).toBe(false);
    });
  });

  describe('SuccessCriteriaSchema', () => {
    it('validates correct success criteria', () => {
      const criteria = {
        description: 'Success',
        requiredArtifacts: ['artifact1'],
      };
      const result = SuccessCriteriaSchema.safeParse(criteria);
      expect(result.success).toBe(true);
    });

    it('validates without optional requiredArtifacts', () => {
      const criteria = {
        description: 'Success',
      };
      const result = SuccessCriteriaSchema.safeParse(criteria);
      expect(result.success).toBe(true);
    });

    it('fails if description is missing', () => {
      const result = SuccessCriteriaSchema.safeParse({});
      expect(result.success).toBe(false);
    });
  });

  describe('Helper Functions', () => {
    const validWorkflowJSON = JSON.stringify({
      steps: [
        {
          stepId: '1',
          description: 'desc',
          action: { toolName: 'tool', purpose: 'purpose' },
          requiredData: [],
          outputVariable: 'out',
        },
      ],
    });
    const invalidJSON = '{"steps": "not an array"}';
    const invalidSyntaxJSON = '{invalid json}';

    const validCriteriaJSON = JSON.stringify({
      description: 'success',
      requiredArtifacts: ['art1'],
    });

    describe('parsePlaybookWorkflow', () => {
      it('parses valid workflow JSON', () => {
        const result = parsePlaybookWorkflow(validWorkflowJSON);
        expect(result.steps).toHaveLength(1);
        expect(result.steps[0].stepId).toBe('1');
      });

      it('throws on invalid schema', () => {
        expect(() => parsePlaybookWorkflow(invalidJSON)).toThrow();
      });

      it('throws on invalid JSON syntax', () => {
        expect(() => parsePlaybookWorkflow(invalidSyntaxJSON)).toThrow();
      });
    });

    describe('parseSuccessCriteria', () => {
      it('parses valid criteria JSON', () => {
        const result = parseSuccessCriteria(validCriteriaJSON);
        expect(result.description).toBe('success');
      });

      it('throws on invalid schema', () => {
        expect(() => parseSuccessCriteria('{"not_desc": "fail"}')).toThrow();
      });

      it('throws on invalid JSON syntax', () => {
        expect(() => parseSuccessCriteria(invalidSyntaxJSON)).toThrow();
      });
    });

    describe('safeParsePlaybookWorkflow', () => {
      it('returns parsed workflow for valid JSON', () => {
        const result = safeParsePlaybookWorkflow(validWorkflowJSON);
        expect(result).toBeDefined();
        expect(result?.steps).toHaveLength(1);
      });

      it('returns undefined for invalid schema', () => {
        expect(safeParsePlaybookWorkflow(invalidJSON)).toBeUndefined();
      });

      it('returns undefined for invalid JSON syntax', () => {
        expect(safeParsePlaybookWorkflow(invalidSyntaxJSON)).toBeUndefined();
      });
    });

    describe('safeParseSuccessCriteria', () => {
      it('returns parsed criteria for valid JSON', () => {
        const result = safeParseSuccessCriteria(validCriteriaJSON);
        expect(result).toBeDefined();
        expect(result?.description).toBe('success');
      });

      it('returns undefined for invalid schema', () => {
        expect(safeParseSuccessCriteria('{"not_desc": "fail"}')).toBeUndefined();
      });

      it('returns undefined for invalid JSON syntax', () => {
        expect(safeParseSuccessCriteria(invalidSyntaxJSON)).toBeUndefined();
      });
    });

    describe('Session Pinning & Multi-Session Routing', () => {
      it('validates a step with targetSession override and promptTemplate', () => {
        const stepWithSession = {
          stepId: 'analyze',
          description: 'Run deep data analysis',
          action: {
            toolName: 'agent__spawnSession',
            purpose: 'Analyze collected data',
          },
          targetSession: {
            mode: 'spawn' as const,
            configId: 'analyst-config',
            sessionSlot: 'analyst',
          },
          sessionSlot: 'analyst',
          promptTemplate: 'Please analyze this data: {raw_data}',
          requiredData: ['raw_data'],
          outputVariable: 'analysis_result',
        };
        const result = PlaybookStepSchema.safeParse(stepWithSession);
        expect(result.success).toBe(true);
        if (result.success) {
          expect(result.data.targetSession?.mode).toBe('spawn');
          expect(result.data.promptTemplate).toBe('Please analyze this data: {raw_data}');
        }
      });

      it('validates workflow with defaultTargetSession and sessionSlots', () => {
        const workflowWithSlots = {
          defaultTargetSession: {
            mode: 'self' as const,
          },
          sessionSlots: {
            analyst: {
              mode: 'spawn' as const,
              configId: 'analyst-cfg',
              reuseAcrossSteps: true,
            },
            reviewer: {
              mode: 'pin' as const,
              sessionId: 'pinned-session-123',
            },
          },
          steps: [
            {
              stepId: '1',
              description: 'Initial step in self',
              action: { toolName: 'test', purpose: 'test' },
              requiredData: [],
              outputVariable: 'data',
            },
            {
              stepId: '2',
              sessionSlot: 'analyst',
              description: 'Step in analyst slot',
              promptTemplate: 'Analyze: {data}',
              action: { toolName: 'test', purpose: 'test' },
              requiredData: ['data'],
              outputVariable: 'report',
            },
          ],
        };
        const result = PlaybookWorkflowSchema.safeParse(workflowWithSlots);
        expect(result.success).toBe(true);
        if (result.success) {
          expect(result.data.sessionSlots?.analyst.mode).toBe('spawn');
          expect(result.data.sessionSlots?.reviewer.sessionId).toBe('pinned-session-123');
          expect(result.data.steps[1].sessionSlot).toBe('analyst');
        }
      });

      it('safely parses raw array JSON as workflow with steps', () => {
        const rawArrayJSON = JSON.stringify([
          {
            stepId: 'legacy_1',
            description: 'Legacy array step',
            action: { toolName: 'tool', purpose: 'purpose' },
            requiredData: [],
            outputVariable: 'res',
          },
        ]);
        const result = safeParsePlaybookWorkflow(rawArrayJSON);
        expect(result).toBeDefined();
        expect(result?.steps).toHaveLength(1);
        expect(result?.steps[0].stepId).toBe('legacy_1');
      });
    });

    describe('interpolateVariables', () => {
      it('replaces single and multiple placeholders with matching values', () => {
        const template = 'Analyze {data} and generate {format} report.';
        const vars = { data: 'Q3_metrics.csv', format: 'PDF' };
        expect(interpolateVariables(template, vars)).toBe(
          'Analyze Q3_metrics.csv and generate PDF report.',
        );
      });

      it('leaves unmatched placeholders untouched', () => {
        const template = 'Hello {user}, your token is {token} and balance is {balance}.';
        const vars = { user: 'Alice' };
        expect(interpolateVariables(template, vars)).toBe(
          'Hello Alice, your token is {token} and balance is {balance}.',
        );
      });
    });
  });
});
