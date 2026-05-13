1. **Request Review of the Plan**
   - Use `request_plan_review` to have the plan reviewed.

2. **Modify `list_agents_or_sessions` in `configs.rs`**
   - Use `replace_with_git_merge_diff` to update `list_agents_or_sessions` to pass `&args` into `list_delegated_sessions`.
   - Update `list_delegated_sessions` to accept `args: &Value`.

3. **Implement in-memory pagination for `list_delegated_sessions`**
   - Use `replace_with_git_merge_diff` to extract `limit` and `offset` from `args` using `.unwrap_or` fallbacks (`limit: 20`, `offset: 0`).
   - Use `.into_iter().skip(offset).take(limit).collect()` to page the items without modifying the underlying database query.
   - Adjust the Markdown table summary to mention `total` count.

4. **Append truncation hints for Distill protocol**
   - Use `replace_with_git_merge_diff` to append `\n*(Showing X to Y of Z items. Call this tool again with offset: N to see more)*` in `list_agent_configs` and `list_delegated_sessions`.

5. **Verify Changes**
   - Use `run_in_bash_session` to run `cargo check -p libragent` to ensure types align and code compiles.

6. **Journal Entry**
   - Use `run_in_bash_session` to append a Distill protocol journal entry to `.jules/distill.md`.

7. **Complete pre-commit steps**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

8. **Submit**
   - Use `run_in_bash_session` with `gh pr create` with proper title and specific Distill protocol format.
