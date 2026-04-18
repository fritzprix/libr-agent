# High-Quality Playbook Patterns

Use these patterns to design robust, reusable workflows.

## 1. Information Extraction & Processing

**Goal**: Analyze structured or unstructured data and produce a summary.

### Success Criteria:
- **Description**: A comprehensive analysis report is generated.
- **Artifacts**: `analysis_report.md`

### Workflow Pattern:
1. **Discovery**: `workspace__listDirectory` to find files. (output: `file_list`)
2. **Extraction**: `workspace__readFile` to read target content. (output: `raw_data`)
3. **Synthesis**: (Agent internal reasoning) Use `raw_data` to generate report. (output: `synthesis`)
4. **Finalization**: `workspace__writeFile` to save `analysis_report.md`.

---

## 2. Iterative Development & Validation

**Goal**: Develop, test, and fix a feature or bug.

### Success Criteria:
- **Description**: Feature implemented and tests passed.
- **Artifacts**: `src/feature.py`, `tests/results.log`

### Workflow Pattern:
1. **Implementation**: `workspace__writeFile` to create code. (output: `feature_code`)
2. **Testing**: `workspace__runPowerShell` to execute tests. (output: `test_output`)
3. **Analysis**: Use `test_output` to identify failures.
4. **Fix**: `workspace__editFiles` to correct errors based on analysis.
5. **Verification**: `workspace__runPowerShell` to re-run tests.

---

## 3. Environment Setup & Deployment

**Goal**: Configure a workspace for a specific project type.

### Success Criteria:
- **Description**: Environment is ready for development.
- **Artifacts**: `.env`, `node_modules/`

### Workflow Pattern:
1. **Bootstrap**: `workspace__runPowerShell` to install dependencies (e.g., `npm install`).
2. **Configuration**: `workspace__writeFile` to create `.env` from template.
3. **Validation**: `workspace__runPowerShell` to check environment health (e.g., `npm test`).

---

## Design Principles for Playbooks

1. **Granularity**: Break steps into atomic actions. Don't "Read, Analyze, and Write" in one step.
2. **Robustness**: Use `requiredData` to ensure steps only run when previous dependencies are met.
3. **Traceability**: Give `outputVariable` names that clearly describe the data they hold (e.g., `config_json`, `search_results`).
4. **Tool Appropriateness**: Use the right tool for the job. Use `runPowerShell` for complex shell logic and `editFiles` for precise code modifications.
