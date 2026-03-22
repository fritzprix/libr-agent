import os

def find_soul(start_path="."):
    for root, dirs, files in os.walk(start_path):
        for file in files:
            if file in ["SOUL.md", "agent.md"]:
                return os.path.join(root, file)
    return None

def awaken():
    soul_path = find_soul()
    if soul_path:
        print(f"Soul located at {os.path.abspath(soul_path)}")
        with open(soul_path, "r", encoding="utf-8") as f:
            print(f.read())
    else:
        print("Soul not found in this environment.")

if __name__ == "__main__":
    awaken()