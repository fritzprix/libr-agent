//! Streak counting over trailing tool-call turns: identical (call, outcome) runs,
//! repeated batches, and the wait state that splits them into wait and fast runs.

use super::outcome_signature::{
    build_batch_outcome_signature, build_tool_result_signature, is_loop_prevention_message,
    is_tool_error_message, repeated_outcome_key, tool_result_duration_ms, RepeatedOutcome,
};
use super::settings::WaitPreventionSettings;
use super::signature::batch_fingerprint;
use crate::models::chat::Message;

/// Trailing assistant tool-call turns, newest first.
///
/// Stops at the first non-tool / non-tool-calling-assistant message (e.g. user).
struct ToolCallTurn<'a> {
    assistant_message: &'a Message,
    tool_results: Vec<&'a Message>,
}

fn group_trailing_tool_call_turns(messages: &[Message]) -> Vec<ToolCallTurn<'_>> {
    let mut turns: Vec<ToolCallTurn> = Vec::new();
    let mut current_tool_results: Vec<&Message> = Vec::new();

    for message in messages.iter().rev() {
        match message.role.as_str() {
            "tool" => {
                current_tool_results.push(message);
            }
            "assistant" if message.tool_calls.is_some() => {
                turns.push(ToolCallTurn {
                    assistant_message: message,
                    tool_results: std::mem::take(&mut current_tool_results),
                });
            }
            _ => {
                break;
            }
        }
    }

    turns
}

/// Whether a trailing turn extends an identical-call wait streak (#2610).
#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitState {
    /// Result duration reached the wait threshold.
    Slow,
    /// Loop-prevention short-circuit or missing result: keeps an open wait
    /// streak alive without starting one.
    Neutral,
    /// Fast result: closes the wait streak; the fast-loop rule owns it.
    Fast,
}

fn fold_wait_state(
    state: WaitState,
    slow_suffix: &mut usize,
    pending_neutral: &mut usize,
    closed: &mut bool,
) {
    if *closed {
        return;
    }
    match state {
        WaitState::Slow => {
            *slow_suffix += *pending_neutral + 1;
            *pending_neutral = 0;
        }
        WaitState::Neutral => *pending_neutral += 1,
        WaitState::Fast => *closed = true,
    }
}

/// Count trailing tool results whose call signature matches `matcher`.
///
/// Counts only the trailing streak of identical (call, outcome) pairs:
/// - A different tool call (name/args) ends the streak.
/// - A different tool-result signature also ends the streak (allows legitimate
///   polling where status progresses, e.g. running → completed).
/// - Loop-prevention short-circuit results keep the streak (they must not look
///   like a “new outcome” that clears the counter).
///
/// Also returns the number of trailing **slow** (wait) calls: the longest
/// suffix of the streak whose results are slow, counting loop-prevention /
/// missing results only when they sit above an already-open wait streak.
///
/// And the number of trailing **fast** calls: the newest run of turns that is
/// not part of a wait streak (loop-prevention / missing results above an open
/// wait streak count as fast). The fast-loop ladder must count only this
/// suffix — a long wait streak followed by one fast identical result must not
/// inherit the wait history as fast-loop attempts (#2610 G1.3). A turn whose
/// batch contained a wait (any member slow, #2610 G3.3) is not a fast attempt
/// either: it belongs to the batch wait ladder, not the per-tool fast ladder.
///
/// Finally, the **total** identical (call, outcome) streak length regardless
/// of duration — the bound for the overall hard cap (#2610 G3.1).
pub(super) fn count_consecutive_identical_call_outcomes<F>(
    messages: &[Message],
    matcher: F,
    wait: &WaitPreventionSettings,
) -> Option<(usize, usize, usize, RepeatedOutcome)>
where
    F: Fn(&str) -> bool,
{
    let turns = group_trailing_tool_call_turns(messages);

    let mut consecutive_matches = 0;
    let mut repeated_outcome: Option<RepeatedOutcome> = None;
    let mut slow_suffix: usize = 0;
    let mut fast_suffix: usize = 0;
    let mut wait_streak_seen = false;
    let mut pending_neutral: usize = 0;
    let mut wait_closed = false;

    for (turn_index, turn) in turns.iter().enumerate() {
        let mut matched_in_turn = false;
        let mut turn_outcome: Option<RepeatedOutcome> = None;
        let mut turn_wait_state = WaitState::Fast;

        if let Some(tool_calls) = &turn.assistant_message.tool_calls {
            for tool_call in tool_calls {
                if matcher(&tool_call.id) {
                    matched_in_turn = true;
                    if let Some(tool_result_msg) = turn
                        .tool_results
                        .iter()
                        .find(|m| m.tool_call_id.as_deref() == Some(&tool_call.id))
                    {
                        if is_loop_prevention_message(tool_result_msg) {
                            turn_wait_state = WaitState::Neutral;
                        } else {
                            if tool_result_duration_ms(tool_result_msg)
                                .is_some_and(|duration| duration >= wait.wait_duration_ms)
                            {
                                turn_wait_state = WaitState::Slow;
                            }
                            let signature = build_tool_result_signature(tool_result_msg);
                            turn_outcome = Some(if is_tool_error_message(tool_result_msg) {
                                RepeatedOutcome::Error { signature }
                            } else {
                                RepeatedOutcome::Success { signature }
                            });
                        }
                    } else {
                        turn_wait_state = WaitState::Neutral;
                    }
                }
            }
        }

        if !matched_in_turn {
            break;
        }

        if let Some(outcome) = turn_outcome {
            match &repeated_outcome {
                None => {
                    repeated_outcome = Some(outcome);
                    consecutive_matches += 1;
                }
                Some(prev) if repeated_outcome_key(prev) == repeated_outcome_key(&outcome) => {
                    consecutive_matches += 1;
                }
                Some(_) => {
                    // Outcome changed — stop at the trailing identical-outcome segment.
                    break;
                }
            }
        } else {
            // Loop-prevention or missing result: keep the streak alive.
            consecutive_matches += 1;
        }

        // A turn the batch wait ladder owned when it ran (#2610 G3.3) is
        // invisible to this member's per-tool ladders: neither a fast attempt
        // nor a wait of its own — the batch ladder accounted for it (and
        // would have guided the whole batch together past the guide count).
        // It still counts toward the overall total-streak cap above.
        if turn_owned_by_batch_ladder(&turns, turn_index, wait) {
            continue;
        }

        if !wait_streak_seen {
            if turn_wait_state == WaitState::Slow {
                wait_streak_seen = true;
            } else {
                fast_suffix += 1;
            }
        }

        fold_wait_state(
            turn_wait_state,
            &mut slow_suffix,
            &mut pending_neutral,
            &mut wait_closed,
        );
    }

    if consecutive_matches == 0 {
        return None;
    }

    let outcome = repeated_outcome.unwrap_or_else(|| RepeatedOutcome::Error {
        signature: "__loop_prevention__".to_string(),
    });
    Some((slow_suffix, fast_suffix, consecutive_matches, outcome))
}

/// Minimum trailing assistant tool_calls batches to inspect when computing a
/// batch-repetition streak. Callers scan at least this many batches — or far
/// enough back to reach the configured wait hard-break
/// (`guide_count + break_count + 1`), whichever is larger — bounding work on
/// very long sessions. User/system messages still reset the streak.
const MAX_ASSISTANT_BATCHES_TO_SCAN: usize = 32;

/// Count consecutive prior assistant batches whose fingerprint matches `fingerprint`
/// **and** whose complete tool-result outcomes match the trailing outcome segment.
///
/// Returns `(consecutive, slow_suffix, fast_suffix)`: `slow_suffix` is the
/// newest run of wait batches (see [`fold_wait_state`]); `fast_suffix` is the
/// newest run of non-wait batches (incomplete/loop-prevention batches above an
/// open wait streak count as fast). The wait ladder uses `slow_suffix`, the
/// fast ladder uses `fast_suffix` (#2610 G1.3).
///
/// Intervening incomplete or loop-prevention results keep the streak. A different
/// non-empty tool_calls batch, a different complete batch outcome, or a user/system
/// message ends the streak. This catches `[a,b,c] → [a,b,c]` which the per-tool
/// result scan misses, while allowing progressive polling across repeated batches.
///
/// `min_scan` raises the scan floor so callers can reach their own hard break
/// (e.g. a fast hard break at `threshold + hard_break_offset` above 32).
pub fn count_consecutive_identical_batches(
    messages: &[Message],
    fingerprint: &str,
    wait: &WaitPreventionSettings,
    min_scan: usize,
) -> (usize, usize, usize) {
    batch_streak_stats(
        &group_trailing_tool_call_turns(messages),
        fingerprint,
        wait,
        min_scan,
    )
}

/// [`count_consecutive_identical_batches`] core over already-grouped turns
/// (newest first), so per-turn ownership checks can run on history slices
/// without re-grouping (#2610 G3.3).
fn batch_streak_stats(
    turns: &[ToolCallTurn<'_>],
    fingerprint: &str,
    wait: &WaitPreventionSettings,
    min_scan: usize,
) -> (usize, usize, usize) {
    let mut consecutive = 0;
    let mut repeated_batch_outcome: Option<String> = None;
    let mut assistant_batches_scanned = 0;
    let mut slow_suffix: usize = 0;
    let mut fast_suffix: usize = 0;
    let mut wait_streak_seen = false;
    let mut pending_neutral: usize = 0;
    let mut wait_closed = false;
    // Scan far enough back to reach the wait hard break for any configured
    // guide/break counts (#2610 S1.2) and the caller's own hard break
    // (`min_scan`, #2610 G4.1), not just the fixed floor.
    let scan_cap = MAX_ASSISTANT_BATCHES_TO_SCAN
        .max(wait.guide_count + wait.break_count + 1)
        .max(min_scan);

    for turn in turns {
        let Some(tool_calls) = turn.assistant_message.tool_calls.as_ref() else {
            break;
        };
        if tool_calls.is_empty() {
            break;
        }

        assistant_batches_scanned += 1;
        if assistant_batches_scanned > scan_cap {
            break;
        }

        if batch_fingerprint(tool_calls) != fingerprint {
            break;
        }

        if let Some(outcome) = build_batch_outcome_signature(tool_calls, &turn.tool_results) {
            // A batch counts as a wait when any of its results took the wait
            // threshold: polling batches usually contain at least one blocking wait.
            let wait_state = if turn.tool_results.iter().any(|result| {
                tool_result_duration_ms(result).is_some_and(|d| d >= wait.wait_duration_ms)
            }) {
                WaitState::Slow
            } else {
                WaitState::Fast
            };
            match &repeated_batch_outcome {
                None => {
                    repeated_batch_outcome = Some(outcome);
                    consecutive += 1;
                }
                Some(prev) if prev == &outcome => {
                    consecutive += 1;
                }
                Some(_) => {
                    break;
                }
            }
            if wait_streak_seen {
                // Below the newest wait batch: the trailing fast run is already fixed.
            } else if wait_state == WaitState::Slow {
                wait_streak_seen = true;
            } else {
                fast_suffix += 1;
            }
            fold_wait_state(
                wait_state,
                &mut slow_suffix,
                &mut pending_neutral,
                &mut wait_closed,
            );
        } else {
            // Incomplete results or loop-prevention: keep the streak.
            consecutive += 1;
            if !wait_streak_seen {
                fast_suffix += 1;
            }
            fold_wait_state(
                WaitState::Neutral,
                &mut slow_suffix,
                &mut pending_neutral,
                &mut wait_closed,
            );
        }
    }

    (consecutive, slow_suffix, fast_suffix)
}

/// Whether the batch wait ladder owned this turn when it ran (#2610 G3.3).
///
/// Mirrors the runtime gate ([`super::is_wait_batch_streak`]) as it would have been
/// evaluated *before* this turn executed: the same batch repeated with an
/// identical outcome at least twice in the older history, with a trailing
/// wait run. Turn members that the batch ladder already owned (and, when the
/// streak crossed the guide count, guided together) must not feed a member's
/// per-tool fast streak afterwards — the per-tool scan resuming later would
/// otherwise inherit them as fast attempts and hard-break with no warning.
fn turn_owned_by_batch_ladder(
    turns: &[ToolCallTurn<'_>],
    index: usize,
    wait: &WaitPreventionSettings,
) -> bool {
    let Some(tool_calls) = turns[index].assistant_message.tool_calls.as_ref() else {
        return false;
    };
    // Mirror `is_wait_batch_streak`'s exclusions exactly.
    if tool_calls.len() < 2 {
        return false;
    }
    let first = &tool_calls[0];
    if first.function.name == "ui__circuitBreak" || first.function.name == "planning__reflect" {
        return false;
    }

    let fingerprint = batch_fingerprint(tool_calls);
    let (previous, slow_suffix, _) = batch_streak_stats(&turns[index + 1..], &fingerprint, wait, 0);
    batch_wait_ladder_owns(previous, slow_suffix)
}

/// Shared gate for [`super::is_wait_batch_streak`] and [`turn_owned_by_batch_ladder`]
/// so the runtime check and the history replay cannot drift apart (#2610 S4.2).
///
/// G3.2: members are exempt only while the whole batch is actually repeating
/// with an identical outcome. A single slow batch whose outcome differs from
/// the turn before never forms a wait streak.
pub(super) fn batch_wait_ladder_owns(previous: usize, slow_suffix: usize) -> bool {
    previous >= 2 && slow_suffix > 0
}
