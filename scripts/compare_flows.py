import json
import re
import sys
from collections import defaultdict
from datetime import datetime

def parse_n_log(file_path):
    """
    Parses n_log.txt to extract MessageAdded events.
    Returns a dictionary mapping session_id to a list of messages.
    """
    sessions = defaultdict(list)
    
    # Regex to capture the start of a MessageAdded event
    # [2026-01-26][11:13:18][tauri_mcp_agent_lib::agent::events][INFO] Emitting agent event: MessageAdded {
    event_start_re = re.compile(r'Emitting agent event: MessageAdded \{')
    
    # Regexes for fields within the struct
    session_id_re = re.compile(r'session_id: "([^"]+)"')
    id_re = re.compile(r'\bid: "([^"]+)"')
    role_re = re.compile(r'role: "([^"]+)"')
    
    current_message = {}
    in_message_block = False
    
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            lines = f.readlines()
            
        events = []
        current_event = None
        
        for line in lines:
            if "Emitting agent event: MessageAdded {" in line:
                if current_event:
                    events.append(current_event)
                current_event = {'raw': []}
            
            if current_event is not None:
                current_event['raw'].append(line)
                # Check if this line looks like the start of a new log entry (timestamp)
                # [2026-01-26]...
                if line.startswith('[') and "Emitting agent event: MessageAdded {" not in line and len(current_event['raw']) > 1:
                     # This is a new log line, so the previous event block is done
                     events.append(current_event)
                     current_event = None

        if current_event:
            events.append(current_event)
            
        # Now process the captured blocks
        for event in events:
            block = "".join(event['raw'])
            
            # Extract info
            sid = session_id_re.search(block)
            mid = id_re.search(block)
            role = role_re.search(block)
            
            if sid and mid and role:
                msg = {
                    'session_id': sid.group(1),
                    'id': mid.group(1),
                    'role': role.group(1),
                    'source': 'log'
                }
                sessions[msg['session_id']].append(msg)
                
    except Exception as e:
        print(f"Error parsing n_log.txt: {e}")
        
    return sessions

def parse_trace_json(file_path):
    """
    Parses .trace.json to extract messages.
    Returns a dictionary mapping session_id to a list of messages.
    """
    sessions = defaultdict(list)
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            
        if isinstance(data, list):
            for item in data:
                if 'sessionId' in item and 'id' in item:
                    msg = {
                        'session_id': item['sessionId'],
                        'id': item['id'],
                        'role': item.get('role', 'unknown'),
                        'source': 'trace'
                    }
                    sessions[msg['session_id']].append(msg)
    except Exception as e:
        print(f"Error parsing .trace.json: {e}")
        
    return sessions

def compare_sessions(log_sessions, trace_sessions):
    all_session_ids = set(log_sessions.keys()) | set(trace_sessions.keys())
    
    print(f"Total Sessions Found: {len(all_session_ids)}")
    print("-" * 60)
    
    for sid in all_session_ids:
        log_msgs = log_sessions.get(sid, [])
        trace_msgs = trace_sessions.get(sid, [])
        
        print(f"Session ID: {sid}")
        print(f"  Log Messages: {len(log_msgs)}")
        print(f"  Trace Messages: {len(trace_msgs)}")
        
        log_ids = [m['id'] for m in log_msgs]
        trace_ids = [m['id'] for m in trace_msgs]
        
        # Check for missing in Trace
        missing_in_trace = set(log_ids) - set(trace_ids)
        if missing_in_trace:
            print(f"  [WARNING] Missing in Trace ({len(missing_in_trace)}): {missing_in_trace}")
            
        # Check for missing in Log
        missing_in_log = set(trace_ids) - set(log_ids)
        if missing_in_log:
            print(f"  [INFO] Extra in Trace (not in Log) ({len(missing_in_log)}): {missing_in_log}")
            
        # Check Order
        # We can only compare order for common IDs
        common_ids = [mid for mid in log_ids if mid in trace_ids]
        
        # Filter trace to only common
        trace_common_ordered = [mid for mid in trace_ids if mid in set(common_ids)]
        
        if common_ids != trace_common_ordered:
             print(f"  [WARNING] Order Mismatch!")
             print(f"    Log Order (Common): {common_ids}")
             print(f"    Trace Order (Common): {trace_common_ordered}")
        else:
             print(f"  [OK] Order Matches for {len(common_ids)} common messages.")
             
        print("-" * 60)

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python compare_flows.py <n_log.txt> <.trace.json>")
        sys.exit(1)
        
    log_file = sys.argv[1]
    trace_file = sys.argv[2]
    
    print(f"Parsing Log: {log_file}")
    log_data = parse_n_log(log_file)
    
    print(f"Parsing Trace: {trace_file}")
    trace_data = parse_trace_json(trace_file)
    
    compare_sessions(log_data, trace_data)
