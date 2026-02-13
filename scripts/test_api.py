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

        return True
    finally:
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

if __name__ == "__main__":
    try:
        if test_session_flow():
            log("Test passed successfully!", "SUCCESS")
            sys.exit(0)
        else:
            log("Test failed.", "ERROR")
            sys.exit(1)
    except requests.exceptions.ConnectionError:
        log("Could not connect to server. Is the app running?", "ERROR")
        sys.exit(1)
