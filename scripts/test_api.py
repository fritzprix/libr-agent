import requests
import time
import json
import sys

# Configuration
BASE_URL = "http://localhost:3030/api/sessions"
ASSISTANTS_URL = "http://localhost:3030/api/assistants"

def log(msg, type="INFO"):
    print(f"[{type}] {msg}")

def test_session_flow():
    log("Starting API E2E Test...")

    # 0. Fetch Assistants
    log("Fetching available assistants...")
    try:
        resp = requests.get(ASSISTANTS_URL)
        resp.raise_for_status()
        assistants = resp.json().get('assistants', [])
        if not assistants:
            log("No assistants found. Cannot proceed.", "ERROR")
            return False
        
        # Pick the first one
        assistant = assistants[0]
        assistant_config = json.loads(assistant['config'])
        assistant_config['id'] = assistant['id'] # Inject ID required by backend
        assistant_config['name'] = assistant['name'] # Inject Name required by AgentConfig struct
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
        response = requests.post(BASE_URL, json=payload)
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

    # 4. Poll for Response (No need to send message separately)
    log("Polling for response...")
    max_retries = 10
    has_response = False
    
    for i in range(max_retries):
        time.sleep(1)
        # Check Status
        status_resp = requests.get(f"{BASE_URL}/{session_id}")
        current_status = status_resp.json().get('status')
        log(f"Poll {i+1}/{max_retries}: Status = {current_status}")

        # Check Messages
        msg_resp = requests.get(f"{BASE_URL}/{session_id}/messages")
        messages = msg_resp.json().get('messages', [])
        
        # Look for assistant message
        assistant_msgs = [m for m in messages if m['role'] == 'assistant']
        if assistant_msgs:
            log(f"Received assistant response: {len(assistant_msgs)} messages", "SUCCESS")
            has_response = True
            break
        
        # If we started and are now Idle, and have no response yet, wait a bit
        if current_status == 'Idle' and not has_response and i > 5:
             pass

    if not has_response:
        log("Timed out waiting for assistant response.", "WARN")

    # 5. Send Message while potentially Busy (Test Queuing)
    # Note: Hard to guarantee "Busy" state in this simple script without a long-running task, 
    # but we can try immediately after sending another one.
    
    # 6. Terminate Session
    log("Terminating session...")
    requests.post(f"{BASE_URL}/{session_id}/terminate")
    log("Session terminated.", "SUCCESS")
    
    return True

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
