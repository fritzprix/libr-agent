import sys
try:
    with open('error.txt', 'rb') as f:
        content = f.read()
        print(f"Size: {len(content)}")
        print(f"Has NUL: {b'\x00' in content}")
        print("Content repr:")
        print(content)
except Exception as e:
    print(f"Error: {e}")
