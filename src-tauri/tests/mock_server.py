import json
import os
import sys

def main():
    cwd_capture_file = os.environ.get("MOCK_SERVER_CWD_FILE")
    if cwd_capture_file:
        with open(cwd_capture_file, "w", encoding="utf-8") as capture_file:
            capture_file.write(os.getcwd())

    # Two backends on one profile must each own a child; the pid file lets the
    # test check that the children are distinct and that stopping one backend
    # leaves the other's child running (#2858 S6).
    pid_capture_file = os.environ.get("MOCK_SERVER_PID_FILE")
    if pid_capture_file:
        with open(pid_capture_file, "w", encoding="utf-8") as capture_file:
            capture_file.write(str(os.getpid()))

    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break
            
            try:
                request = json.loads(line)
            except json.JSONDecodeError:
                continue

            if request.get("method") == "initialize":
                response = {
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "serverInfo": {
                            "name": "mock-server",
                            "version": "1.0"
                        }
                    }
                }
                sys.stdout.write(json.dumps(response) + "\n")
                sys.stdout.flush()
            elif request.get("method") == "tools/list":
                response = {
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "tools": []
                    }
                }
                sys.stdout.write(json.dumps(response) + "\n")
                sys.stdout.flush()
            
            # Keep the process alive for other messages or just idle
        except KeyboardInterrupt:
            break

if __name__ == "__main__":
    main()
