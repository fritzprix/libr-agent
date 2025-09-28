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
 * A reusable playbook for achieving a single goal. Playbooks represent
 * successful agent workflows that can be re-used or replayed.
 */
export interface Playbook {
  /** Identifier of the agent suitable for performing this playbook */
  agentId: string;

  /** The final goal this playbook aims to achieve */
  goal: string;

  /** Stores the user's initial natural language command as-is */
  initialCommand: string;

  /** Set of sequential steps for achieving the goal */
  workflow: PlaybookStep[];

  /**
   * Describes the objective criteria for this playbook to be considered 'successful'
   */
  successCriteria: {
    // 기존의 설명
    description: string;

    // (추가) Task 성공 시 반드시 생성되어야 하는 파일 목록
    requiredArtifacts?: string[]; // 예: ["report.pdf", "summary.txt"]
  };
}
