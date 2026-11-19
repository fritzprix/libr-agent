"""2565 O-7 (Windows): docs skill doctor reports 'Doctor: OK' with no junction loop."""

from pathlib import Path

from verifier_lib import (
    load_trajectory,
    observation_result_texts,
    require_session_evidence,
    trajectory_steps,
)

OUTPUT_FILE = Path("/app/o7_doctor_output.txt")


def test_docs_doctor_ok_without_junction_loop():
    require_session_evidence()
    assert OUTPUT_FILE.is_file(), "o7_doctor_output.txt missing"
    output = OUTPUT_FILE.read_text(encoding="utf-8", errors="replace")
    assert "Doctor: OK" in output, f"expected 'Doctor: OK' in doctor output, got: {output[:500]!r}"

    lower = output.lower()
    assert "os error 448" not in lower and "untrusted mount point" not in lower, (
        f"junction/mount error detected in doctor output: {output[:500]!r}"
    )

    obs = "\n".join(observation_result_texts(trajectory_steps(load_trajectory())))
    assert "Doctor: OK" in obs or "o7_doctor_output.txt" in obs, (
        "expected doctor execution evidence in trajectory observations"
    )
