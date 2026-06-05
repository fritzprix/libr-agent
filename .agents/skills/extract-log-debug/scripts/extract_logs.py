#!/usr/bin/env python3
"""
Extract and analyze LibrAgent log files with pattern matching and context.

Usage:
    python extract_logs.py [OPTIONS]
    
Examples:
    # Extract last 100 lines
    python extract_logs.py -n 100
    
    # Extract all errors with context
    python extract_logs.py --pattern "[ERROR]" --context 5
    
    # Extract planning logs
    python extract_logs.py --pattern "PLANNING" -n 5000
    
    # Extract to specific output file
    python extract_logs.py --pattern "[WARN]" -o warnings.txt
"""

import os
import sys
import argparse
from pathlib import Path

# Prevent encoding errors on Windows console
if hasattr(sys.stdout, 'reconfigure'):
    sys.stdout.reconfigure(encoding='utf-8')
if hasattr(sys.stderr, 'reconfigure'):
    sys.stderr.reconfigure(encoding='utf-8')



def get_log_path():
    """Get platform-specific Tauri log path."""
    app_id = "com.fritzprix.libragent"
    log_file = "libragent.log"
    
    if sys.platform == "win32":
        local_appdata = os.environ.get("LOCALAPPDATA") or os.path.join(
            os.path.expanduser("~"), "AppData", "Local"
        )
        return Path(local_appdata) / app_id / "logs" / log_file
    elif sys.platform == "darwin":
        return Path.home() / "Library" / "Logs" / app_id / log_file
    else:  # Linux
        return Path.home() / ".local" / "share" / app_id / "logs" / log_file


def extract_by_pattern(lines, pattern, context_count=5):
    """Extract lines matching pattern with surrounding context."""
    match_ranges = []
    
    # Find all matches
    for i, line in enumerate(lines):
        if pattern in line:
            start = max(0, i - context_count)
            end = min(len(lines) - 1, i + context_count)
            match_ranges.append({"start": start, "end": end, "match_idx": i})
    
    if not match_ranges:
        return [f'No matches found for pattern "{pattern}"']
    
    # Merge overlapping ranges
    merged = []
    if match_ranges:
        current = match_ranges[0].copy()
        for next_range in match_ranges[1:]:
            if next_range["start"] <= current["end"] + 1:
                current["end"] = max(current["end"], next_range["end"])
            else:
                merged.append(current)
                current = next_range.copy()
        merged.append(current)
    
    # Format output
    result = []
    for idx, range_info in enumerate(merged):
        result.append(f"=== Match {idx + 1}: Lines {range_info['start'] + 1}-{range_info['end'] + 1} ===")
        for i in range(range_info["start"], range_info["end"] + 1):
            line_num = str(i + 1).rjust(6)
            marker = " > " if pattern in lines[i] else "   "
            result.append(f"{line_num}{marker}{lines[i].rstrip()}")
        result.append("")
    
    return result


def main():
    parser = argparse.ArgumentParser(
        description="Extract and analyze LibrAgent logs",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  Extract last 100 lines:
    python extract_logs.py -n 100
    
  Extract errors with context:
    python extract_logs.py --pattern "[ERROR]" --context 5
    
  Extract planning logs:
    python extract_logs.py --pattern "PLANNING" -n 5000
        """
    )
    
    parser.add_argument(
        "-n", "--lines",
        type=int,
        help="Number of lines to extract from end of log (default: all)"
    )
    parser.add_argument(
        "--pattern",
        help="Pattern to search for (e.g., '[ERROR]', 'PLANNING', 'agent_')"
    )
    parser.add_argument(
        "--context",
        type=int,
        default=5,
        help="Lines of context around matches (default: 5)"
    )
    parser.add_argument(
        "-o", "--output",
        default="log.txt",
        help="Output file name (default: log.txt)"
    )
    parser.add_argument(
        "--log-path",
        help="Custom log file path (overrides default Tauri location)"
    )
    
    args = parser.parse_args()
    
    # Get log file path
    log_path = Path(args.log_path) if args.log_path else get_log_path()
    
    if not log_path.exists():
        print(f"❌ Log file not found: {log_path}", file=sys.stderr)
        print("\nExpected location:", file=sys.stderr)
        print(f"  {log_path}", file=sys.stderr)
        return 1
    
    print(f"📂 Reading logs from: {log_path}")
    
    # Read log file
    try:
        with open(log_path, "r", encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    except Exception as e:
        print(f"❌ Failed to read log file: {e}", file=sys.stderr)
        return 1
    
    # Extract lines
    if args.lines:
        lines = lines[-args.lines:]
        print(f"📊 Extracted last {args.lines} lines")
    else:
        print(f"📊 Processing {len(lines)} lines")
    
    # Apply pattern filter if specified
    if args.pattern:
        print(f"🔍 Searching for pattern: {args.pattern}")
        output_lines = extract_by_pattern(lines, args.pattern, args.context)
    else:
        output_lines = [line.rstrip() for line in lines]
    
    # Write output
    output_path = Path(args.output)
    try:
        with open(output_path, "w", encoding="utf-8") as f:
            f.write("\n".join(output_lines))
        print(f"✅ Output saved to: {output_path}")
        print(f"   ({len(output_lines)} lines)")
    except Exception as e:
        print(f"❌ Failed to write output: {e}", file=sys.stderr)
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
