import requests
import time
import sys

# Configuration
API_ROOT = "http://localhost:3030/api"
BASE_URL = f"{API_ROOT}/sessions"
ASSISTANTS_URL = f"{API_ROOT}/assistants"
HEALTH_URL = f"{API_ROOT}/health"
REQUEST_TIMEOUT = 10

def log(msg, type="INFO"):
    print(f"[{type}] {msg}")

def test_session_flow():
    log("Starting API E2E Test...")
    session_id = None
    child_session_id = None

    # -1. Health check
    log("Checking API health...")
    try:
        health_resp = requests.get(HEALTH_URL, timeout=REQUEST_TIMEOUT)
        health_resp.raise_for_status()
        health = health_resp.json()
        if health.get("status") != "ok":
            log(f"Unexpected health response: {health}", "ERROR")
            return False
        log("API health check passed.", "SUCCESS")
    except Exception as e:
        log(f"Health check failed: {e}", "ERROR")
        return False

    # 0. Fetch Assistants
    log("Fetching available assistants...")
    try:
        resp = requests.get(ASSISTANTS_URL, timeout=REQUEST_TIMEOUT)
        resp.raise_for_status()
        assistants = resp.json().get('assistants', [])
        if not assistants:
            log("No assistants found. Cannot proceed.", "ERROR")
            return False
        
        # Pick the first one
        assistant = assistants[0]
        log(f"Using assistant: {assistant['name']} ({assistant['id']})", "INFO")
        
    except Exception as e:
        log(f"Failed to fetch assistants: {e}", "ERROR")
        return False

    # 1. Create Session with Initial Message (Create & Execute)
    log("Creating new session with initial request...")
    payload = {
        "name": "E2E Test Session (API + Init Msg)",
        "assistantId": assistant['id'],
        "request": "Hello, this is a test message sent during creation."
    }
    try:
        response = requests.post(BASE_URL, json=payload, timeout=REQUEST_TIMEOUT)
        if response.status_code not in [200, 201]:
             print(f"[ERROR] Failed to create session: Status {response.status_code}")
             print(f"[ERROR] Response body: {response.text}")
             sys.exit(1)
        session_data = response.json()
        session_id = session_data['id']
        # Expect status to be Busy immediately (or Idle if very fast, but ideally Busy)
        initial_status = session_data.get('status')
        log(f"Session created. ID: {session_id}, Initial Status: {initial_status}", "SUCCESS")
        
        # Note: In a real environment, it might finish instantly, but for this test we expect it to start the workflow.
        
    except Exception as e:
        log(f"Failed to create session: {e}", "ERROR")
        return False

    try:
        # 4. Poll for Response (No need to send message separately)
        log("Polling for response...")
        max_retries = 10
        has_response = False

        for i in range(max_retries):
            time.sleep(1)

            # Check Status
            status_resp = requests.get(f"{BASE_URL}/{session_id}", timeout=REQUEST_TIMEOUT)
            status_resp.raise_for_status()
            current_status = status_resp.json().get('status')
            log(f"Poll {i+1}/{max_retries}: Status = {current_status}")

            # Check Messages
            msg_resp = requests.get(
                f"{BASE_URL}/{session_id}/messages", timeout=REQUEST_TIMEOUT
            )
            msg_resp.raise_for_status()
            messages = msg_resp.json().get('messages', [])

            # Look for assistant message
            assistant_msgs = [m for m in messages if m.get('role') == 'assistant']
            if assistant_msgs:
                log(f"Received assistant response: {len(assistant_msgs)} messages", "SUCCESS")
                has_response = True
                break

        if not has_response:
            log("Timed out waiting for assistant response.", "WARN")

        # 5. Create child session from parent session
        log("Creating child session...")
        child_payload = {
            "name": "E2E Child Session",
            "assistantId": assistant["id"],
            "request": "Child session test request.",
            "parentSessionId": session_id,
        }

        child_resp = requests.post(BASE_URL, json=child_payload, timeout=REQUEST_TIMEOUT)
        child_resp.raise_for_status()
        child_data = child_resp.json()
        child_session_id = child_data.get("id")
        child_depth = child_data.get("depth")

        if not child_session_id:
            log(f"Child session creation missing id: {child_data}", "ERROR")
            return False

        if child_depth != 1:
            log(f"Unexpected child depth: {child_depth} (expected 1)", "ERROR")
            return False

        log(
            f"Child session created. ID: {child_session_id}, Depth: {child_depth}",
            "SUCCESS",
        )

        # 6. Verify child list from parent
        log("Checking parent child-session list...")
        children_resp = requests.get(
            f"{BASE_URL}/{session_id}/children", timeout=REQUEST_TIMEOUT
        )
        children_resp.raise_for_status()
        children_data = children_resp.json()
        children = children_data.get("children", [])

        if child_session_id not in children:
            log(
                f"Child session not found in parent children list: {children_data}",
                "ERROR",
            )
            return False

        log("Parent/child lineage check passed.", "SUCCESS")

        return True
    finally:
        if child_session_id:
            log("Terminating child session...")
            try:
                child_term_resp = requests.post(
                    f"{BASE_URL}/{child_session_id}/terminate", timeout=REQUEST_TIMEOUT
                )
                child_term_resp.raise_for_status()
                log("Child session terminated.", "SUCCESS")
            except Exception as e:
                log(f"Failed to terminate child session cleanly: {e}", "WARN")

        if session_id:
            log("Terminating session...")
            try:
                term_resp = requests.post(
                    f"{BASE_URL}/{session_id}/terminate", timeout=REQUEST_TIMEOUT
                )
                term_resp.raise_for_status()
                log("Session terminated.", "SUCCESS")
            except Exception as e:
                log(f"Failed to terminate session cleanly: {e}", "WARN")


def test_max_depth_flow():
    log("Starting maxDepth behavior test...")
    parent_id = None
    child_id = None
    grandchild_id = None

    # 0. Fetch Assistants
    try:
        resp = requests.get(ASSISTANTS_URL, timeout=REQUEST_TIMEOUT)
        resp.raise_for_status()
        assistants = resp.json().get("assistants", [])
        if not assistants:
            log("No assistants found. Cannot proceed maxDepth test.", "ERROR")
            return False
        assistant = assistants[0]
        assistant_id = assistant["id"]
    except Exception as e:
        log(f"Failed to fetch assistants for maxDepth test: {e}", "ERROR")
        return False

    try:
        # 1) Root with maxDepth=1
        log("Creating root session with maxDepth=1...")
        root_payload = {
            "name": "E2E MaxDepth Root",
            "assistantId": assistant_id,
            "request": "Root for maxDepth test.",
            "maxDepth": 1,
        }
        root_resp = requests.post(BASE_URL, json=root_payload, timeout=REQUEST_TIMEOUT)
        root_resp.raise_for_status()
        root_data = root_resp.json()

        parent_id = root_data.get("id")
        if not parent_id:
            log(f"Root session creation missing id: {root_data}", "ERROR")
            return False

        if root_data.get("depth") != 0:
            log(f"Unexpected root depth: {root_data.get('depth')} (expected 0)", "ERROR")
            return False

        if root_data.get("maxDepth") != 1:
            log(
                f"Unexpected root maxDepth: {root_data.get('maxDepth')} (expected 1)",
                "ERROR",
            )
            return False

        log(f"Root created. ID: {parent_id}", "SUCCESS")

        # 2) Child allowed at depth=1
        log("Creating child under maxDepth=1 root (should pass)...")
        child_payload = {
            "name": "E2E MaxDepth Child",
            "assistantId": assistant_id,
            "request": "Child at allowed depth.",
            "parentSessionId": parent_id,
        }
        child_resp = requests.post(BASE_URL, json=child_payload, timeout=REQUEST_TIMEOUT)
        child_resp.raise_for_status()
        child_data = child_resp.json()

        child_id = child_data.get("id")
        if not child_id:
            log(f"Child creation missing id: {child_data}", "ERROR")
            return False

        if child_data.get("depth") != 1:
            log(f"Unexpected child depth: {child_data.get('depth')} (expected 1)", "ERROR")
            return False

        if child_data.get("maxDepth") != 1:
            log(
                f"Unexpected child maxDepth inheritance: {child_data.get('maxDepth')} (expected 1)",
                "ERROR",
            )
            return False

        log(f"Child created. ID: {child_id}", "SUCCESS")

        # 3) Grandchild should be rejected (depth=2 > maxDepth=1)
        log("Creating grandchild under maxDepth=1 root (should fail)...")
        grandchild_payload = {
            "name": "E2E MaxDepth Grandchild",
            "assistantId": assistant_id,
            "request": "This should be rejected by maxDepth.",
            "parentSessionId": child_id,
        }
        grandchild_resp = requests.post(
            BASE_URL, json=grandchild_payload, timeout=REQUEST_TIMEOUT
        )

        if grandchild_resp.status_code != 400:
            log(
                f"Expected 400 for maxDepth violation, got {grandchild_resp.status_code}: {grandchild_resp.text}",
                "ERROR",
            )
            return False

        error_text = grandchild_resp.text.lower()
        if "depth limit exceeded" not in error_text and "maxdepth" not in error_text:
            log(
                f"Unexpected maxDepth rejection body: {grandchild_resp.text}",
                "ERROR",
            )
            return False

        log("Grandchild creation rejected as expected (maxDepth enforced).", "SUCCESS")

        # 4) Unlimited default behavior: child + grandchild both allowed
        log("Creating unlimited root session (no maxDepth)...")
        unlimited_root_payload = {
            "name": "E2E Unlimited Root",
            "assistantId": assistant_id,
            "request": "Root for unlimited depth test.",
        }
        unlimited_root_resp = requests.post(
            BASE_URL, json=unlimited_root_payload, timeout=REQUEST_TIMEOUT
        )
        unlimited_root_resp.raise_for_status()
        unlimited_root_data = unlimited_root_resp.json()

        unlimited_root_id = unlimited_root_data.get("id")
        if not unlimited_root_id:
            log(f"Unlimited root creation missing id: {unlimited_root_data}", "ERROR")
            return False

        if unlimited_root_data.get("maxDepth") is not None:
            log(
                f"Expected unlimited root maxDepth to be null, got: {unlimited_root_data.get('maxDepth')}",
                "ERROR",
            )
            return False

        log(f"Unlimited root created. ID: {unlimited_root_id}", "SUCCESS")

        unlimited_child_payload = {
            "name": "E2E Unlimited Child",
            "assistantId": assistant_id,
            "request": "Depth 1 under unlimited root.",
            "parentSessionId": unlimited_root_id,
        }
        unlimited_child_resp = requests.post(
            BASE_URL, json=unlimited_child_payload, timeout=REQUEST_TIMEOUT
        )
        unlimited_child_resp.raise_for_status()
        unlimited_child_data = unlimited_child_resp.json()
        unlimited_child_id = unlimited_child_data.get("id")

        if not unlimited_child_id:
            log(f"Unlimited child creation missing id: {unlimited_child_data}", "ERROR")
            return False

        unlimited_grandchild_payload = {
            "name": "E2E Unlimited Grandchild",
            "assistantId": assistant_id,
            "request": "Depth 2 under unlimited root.",
            "parentSessionId": unlimited_child_id,
        }
        unlimited_grandchild_resp = requests.post(
            BASE_URL, json=unlimited_grandchild_payload, timeout=REQUEST_TIMEOUT
        )
        unlimited_grandchild_resp.raise_for_status()
        unlimited_grandchild_data = unlimited_grandchild_resp.json()
        grandchild_id = unlimited_grandchild_data.get("id")

        if not grandchild_id:
            log(
                f"Unlimited grandchild creation missing id: {unlimited_grandchild_data}",
                "ERROR",
            )
            return False

        if unlimited_grandchild_data.get("depth") != 2:
            log(
                f"Unexpected unlimited grandchild depth: {unlimited_grandchild_data.get('depth')} (expected 2)",
                "ERROR",
            )
            return False

        log("Unlimited depth behavior validated (depth 2 allowed).", "SUCCESS")

        # cleanup unlimited branch sessions (best-effort)
        try:
            requests.post(
                f"{BASE_URL}/{grandchild_id}/terminate", timeout=REQUEST_TIMEOUT
            ).raise_for_status()
            requests.post(
                f"{BASE_URL}/{unlimited_child_id}/terminate", timeout=REQUEST_TIMEOUT
            ).raise_for_status()
            requests.post(
                f"{BASE_URL}/{unlimited_root_id}/terminate", timeout=REQUEST_TIMEOUT
            ).raise_for_status()
        except Exception as e:
            log(f"Unlimited branch cleanup had warnings: {e}", "WARN")

        return True

    finally:
        if child_id:
            try:
                requests.post(f"{BASE_URL}/{child_id}/terminate", timeout=REQUEST_TIMEOUT)
            except Exception as e:
                log(f"Failed to terminate maxDepth child cleanly: {e}", "WARN")

        if parent_id:
            try:
                requests.post(f"{BASE_URL}/{parent_id}/terminate", timeout=REQUEST_TIMEOUT)
            except Exception as e:
                log(f"Failed to terminate maxDepth root cleanly: {e}", "WARN")


def test_max_fanout_flow():
    log("Starting maxFanout behavior test...")
    root_id = None
    first_child_id = None

    try:
        resp = requests.get(ASSISTANTS_URL, timeout=REQUEST_TIMEOUT)
        resp.raise_for_status()
        assistants = resp.json().get("assistants", [])
        if not assistants:
            log("No assistants found. Cannot proceed maxFanout test.", "ERROR")
            return False
        assistant_id = assistants[0]["id"]

        root_payload = {
            "name": "E2E MaxFanout Root",
            "assistantId": assistant_id,
            "request": "Root for maxFanout test.",
            "maxFanout": 1,
        }
        root_resp = requests.post(BASE_URL, json=root_payload, timeout=REQUEST_TIMEOUT)
        root_resp.raise_for_status()
        root_data = root_resp.json()
        root_id = root_data.get("id")

        if not root_id:
            log(f"MaxFanout root creation missing id: {root_data}", "ERROR")
            return False

        if root_data.get("maxFanout") != 1:
            log(
                f"Unexpected root maxFanout: {root_data.get('maxFanout')} (expected 1)",
                "ERROR",
            )
            return False

        first_child_payload = {
            "name": "E2E MaxFanout Child 1",
            "assistantId": assistant_id,
            "request": "First child should pass.",
            "parentSessionId": root_id,
        }
        first_child_resp = requests.post(
            BASE_URL, json=first_child_payload, timeout=REQUEST_TIMEOUT
        )
        first_child_resp.raise_for_status()
        first_child_data = first_child_resp.json()
        first_child_id = first_child_data.get("id")

        if not first_child_id:
            log(f"First child creation missing id: {first_child_data}", "ERROR")
            return False

        second_child_payload = {
            "name": "E2E MaxFanout Child 2",
            "assistantId": assistant_id,
            "request": "Second child should be rejected.",
            "parentSessionId": root_id,
        }
        second_child_resp = requests.post(
            BASE_URL, json=second_child_payload, timeout=REQUEST_TIMEOUT
        )

        if second_child_resp.status_code != 400:
            log(
                f"Expected 400 for maxFanout violation, got {second_child_resp.status_code}: {second_child_resp.text}",
                "ERROR",
            )
            return False

        error_text = second_child_resp.text.lower()
        if "fanout limit exceeded" not in error_text and "maxfanout" not in error_text:
            log(
                f"Unexpected maxFanout rejection body: {second_child_resp.text}",
                "ERROR",
            )
            return False

        log("Second child rejected as expected (maxFanout enforced).", "SUCCESS")
        return True

    except Exception as e:
        log(f"MaxFanout test failed: {e}", "ERROR")
        return False
    finally:
        if first_child_id:
            try:
                requests.post(
                    f"{BASE_URL}/{first_child_id}/terminate", timeout=REQUEST_TIMEOUT
                )
            except Exception as e:
                log(f"Failed to terminate maxFanout child cleanly: {e}", "WARN")

        if root_id:
            try:
                requests.post(f"{BASE_URL}/{root_id}/terminate", timeout=REQUEST_TIMEOUT)
            except Exception as e:
                log(f"Failed to terminate maxFanout root cleanly: {e}", "WARN")

if __name__ == "__main__":
    try:
        session_ok = test_session_flow()
        max_depth_ok = test_max_depth_flow()
        max_fanout_ok = test_max_fanout_flow()

        if session_ok and max_depth_ok and max_fanout_ok:
            log("Test passed successfully!", "SUCCESS")
            sys.exit(0)
        else:
            log("Test failed.", "ERROR")
            sys.exit(1)
    except requests.exceptions.ConnectionError:
        log("Could not connect to server. Is the app running?", "ERROR")
        sys.exit(1)
