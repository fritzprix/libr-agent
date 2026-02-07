import sys
import json

def main():
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
            
            # Keep the process alive for other messages or just idle
        except KeyboardInterrupt:
            break

if __name__ == "__main__":
    main()
