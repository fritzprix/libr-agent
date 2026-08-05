/// Guidance when a processId is missing from the session registry.
///
/// "Not found" ≠ still running. `readProcessOutput` works while Running.
pub fn process_not_found_guidance(available_summary: String) -> Vec<String> {
    vec![
        available_summary,
        "Use workspace__listProcesses() to find a valid processId.".to_string(),
        "workspace__readProcessOutput works while the process is still running — 'not found' means this ID is not in the registry."
            .to_string(),
    ]
}
