/**
 * An individual step that makes up a playbook (workflow).
 * This step provides guidance and direction for problem solving.
 */
export interface PlaybookStep {
  /** Unique identifier for the step */
  stepId: string;

  /** Describes the goal of this step, such as "competitor technology stack analysis" */
  description: string;

  /** Defines the action to be performed in this step */
  action: {
    /** Name of the tool to use (e.g., "webCrawler", "techStackAnalyzer") */
    toolName: string;

    /**
     * Defines the core 'purpose' of using the tool in this step.
     * The execution agent autonomously configures parameters to achieve this purpose.
     */
    purpose: string;
  };

  /**
   * Specifies what data is needed to achieve the above purpose
   */
  requiredData: string[];

  /** Names the output of this step so it can be referenced by other steps */
  outputVariable: string;
}

/**
 * Defines an input parameter for a playbook template
 */
export interface PlaybookInput {
  /** Variable name used in template strings (e.g., "targetCompany") */
  name: string;

  /** Human-readable description of this input */
  description: string;

  /** Data type of the input value */
  type?: 'string' | 'number' | 'boolean';

  /** Optional default value if not provided at runtime */
  defaultValue?: string;
}

/**
 * A reusable playbook template for achieving a goal.
 * Supports parameterization via inputs for flexible re-execution.
 */
export interface Playbook {
  /** Optional unique identifier (auto-generated if omitted) */
  playbookId?: string;

  /** Short, user-facing title for this playbook */
  title: string;

  /** Goal description (supports {{variableName}} templates) */
  goal: string;

  /** Input parameters required to execute this playbook */
  inputs: PlaybookInput[];

  /** Sequential workflow steps */
  workflow: PlaybookStep[];

  /** Success criteria definition */
  successCriteria: {
    description: string;
    requiredArtifacts?: string[];
  };

  // Legacy/optional fields for backward compatibility
  /** Agent ID (optional for template playbooks) */
  agentId?: string;

  /** Example initial command (optional, for documentation) */
  initialCommand?: string;
}
