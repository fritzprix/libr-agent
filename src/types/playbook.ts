/**
 * Launch targeting for Playbook Card Start.
 * `pin` + sessionId = open that existing session and inject selectPlaybook.
 * Omitted / `self` = today's unpinned path (create a new session).
 */
export type PlaybookTargetSessionMode = 'self' | 'pin';

export type PlaybookDefaultTargetSession =
  | { mode: 'self'; sessionId?: string }
  | { mode: 'pin'; sessionId: string };

/**
 * An individual step that makes up a playbook (workflow).
 * This step provides guidance and direction for problem solving.
 *
 * Optional fields mirror Rust `PlaybookStep` (`step_id` / `required_data` as Option).
 */
export interface PlaybookStep {
  /** Unique identifier for the step (optional; may be omitted in stored JSON) */
  stepId?: string;

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
   * Specifies what data is needed to achieve the above purpose.
   * Normalized to `[]` when omitted during schema parse.
   */
  requiredData?: string[];

  /** Names the output of this step so it can be referenced by other steps */
  outputVariable: string;
}

/**
 * A reusable playbook for achieving a single goal. Playbooks represent
 * successful agent workflows that can be re-used or replayed.
 */
export interface Playbook {
  /** Unique identifier for the playbook (optional for creation) */
  id?: string;

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

  /** Whether the playbook is bookmarked by the user */
  isBookmarked?: boolean;

  /**
   * Optional Start launch target. When mode is `pin` and sessionId exists,
   * Start opens that session instead of creating a new one.
   */
  defaultTargetSession?: PlaybookDefaultTargetSession;
}
