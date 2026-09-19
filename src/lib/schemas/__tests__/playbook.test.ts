import { describe, it, expect } from 'vitest';
import {
  PlaybookStepSchema,
  PlaybookWorkflowSchema,
  PlaybookDefaultTargetSessionSchema,
  SuccessCriteriaSchema,
  parsePlaybookWorkflow,
  parseSuccessCriteria,
  safeParsePlaybookWorkflow,
  safeParseSuccessCriteria,
  normalizePlaybookWorkflow,
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

    it('allows omitted stepId and requiredData (Rust Option parity)', () => {
      const result = PlaybookStepSchema.safeParse({
        description: 'Test step',
        action: { toolName: 'test-tool', purpose: 'Testing' },
        outputVariable: 'result',
      });
      expect(result.success).toBe(true);
      if (result.success) {
        expect(result.data.stepId).toBeUndefined();
        expect(result.data.requiredData).toEqual([]);
      }
    });

    it('fails if required fields are missing', () => {
      const result = PlaybookStepSchema.safeParse({ stepId: '1' });
      expect(result.success).toBe(false);
    });
  });

  describe('PlaybookDefaultTargetSessionSchema', () => {
    it('accepts pin with sessionId', () => {
      const result = PlaybookDefaultTargetSessionSchema.safeParse({
        mode: 'pin',
        sessionId: 'sess-1',
      });
      expect(result.success).toBe(true);
    });

    it('rejects pin without sessionId', () => {
      const result = PlaybookDefaultTargetSessionSchema.safeParse({
        mode: 'pin',
      });
      expect(result.success).toBe(false);
    });

    it('accepts self without sessionId', () => {
      const result = PlaybookDefaultTargetSessionSchema.safeParse({
        mode: 'self',
      });
      expect(result.success).toBe(true);
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

      it('parses legacy raw steps arrays', () => {
        const legacy = JSON.stringify([
          {
            stepId: '1',
            description: 'desc',
            action: { toolName: 'tool', purpose: 'purpose' },
            requiredData: [],
            outputVariable: 'out',
          },
        ]);
        const result = safeParsePlaybookWorkflow(legacy);
        expect(result?.steps).toHaveLength(1);
        expect(result?.defaultTargetSession).toBeUndefined();
      });

      it('parses pin launch target from envelope', () => {
        const withPin = JSON.stringify({
          steps: [
            {
              stepId: '1',
              description: 'desc',
              action: { toolName: 'tool', purpose: 'purpose' },
              requiredData: [],
              outputVariable: 'out',
            },
          ],
          defaultTargetSession: { mode: 'pin', sessionId: 'sess-1' },
        });
        const result = safeParsePlaybookWorkflow(withPin);
        expect(result?.defaultTargetSession).toEqual({
          mode: 'pin',
          sessionId: 'sess-1',
        });
      });

      it('keeps steps when pin is invalid and drops the pin', () => {
        const invalidPin = {
          steps: [
            {
              stepId: '1',
              description: 'desc',
              action: { toolName: 'tool', purpose: 'purpose' },
              outputVariable: 'out',
            },
          ],
          defaultTargetSession: { mode: 'pin' },
        };
        const result = normalizePlaybookWorkflow(invalidPin);
        expect(result?.steps).toHaveLength(1);
        expect(result?.defaultTargetSession).toBeUndefined();
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
  });
});
