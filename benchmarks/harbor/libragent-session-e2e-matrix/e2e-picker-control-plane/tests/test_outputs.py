"""#2903 AC1: unattended file-picker inject & cancel control-plane check."""

from pathlib import Path

from verifier_lib import load_evidence, require_session_evidence

MARKER = Path("/app/picker_probe.txt")


def test_picker_control_plane_arms_path_and_cancel():
    require_session_evidence()
    evidence = load_evidence()
    assert evidence.get("controlPlaneConfigured") is True, (
        "E2E control plane must be configured (debug build + LIBRAGENT_E2E_CONTROL=1)"
    )
    assert "pickerProbeError" not in evidence, (
        f"picker control-plane probe failed: {evidence.get('pickerProbeError')!r}"
    )
    probe = evidence.get("pickerProbe") or {}
    assert probe.get("pathArmed") == {"ok": True}, (
        f"expected path arming to return {{ok: true}}, got {probe!r}"
    )
    assert probe.get("cancelArmed") == {"ok": True}, (
        f"expected cancel arming to return {{ok: true}}, got {probe!r}"
    )
    assert MARKER.is_file(), "workspace marker picker_probe.txt missing"
    assert MARKER.read_text(encoding="utf-8").strip() == "PICKER_PROBE_READY"
