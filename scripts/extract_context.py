#!/usr/bin/env python3
"""
extract_context.py

Usage:
  python3 scripts/extract_context.py /path/to/log.txt --pattern "[ERROR]" --context 5

Description:
  Read the file and print each match (line containing the pattern) with N lines of
  context before and after. Overlapping contexts are merged.

Outputs to stdout. You can redirect to a file if desired.
"""

import argparse
import re
import sys
from typing import List, Tuple


def find_match_ranges(lines: List[str], pattern: str, context: int, use_regex: bool) -> List[Tuple[int,int]]:
    matches: List[int] = []
    if use_regex:
        prog = re.compile(pattern)
        for i, line in enumerate(lines):
            if prog.search(line):
                matches.append(i)
    else:
        for i, line in enumerate(lines):
            if pattern in line:
                matches.append(i)

    ranges: List[Tuple[int,int]] = []
    for i in matches:
        start = max(0, i - context)
        end = min(len(lines) - 1, i + context)
        ranges.append((start, end))

    if not ranges:
        return []

    # Merge overlapping/adjacent ranges
    ranges.sort()
    merged: List[Tuple[int,int]] = [ranges[0]]
    for s, e in ranges[1:]:
        last_s, last_e = merged[-1]
        if s <= last_e + 1:
            merged[-1] = (last_s, max(last_e, e))
        else:
            merged.append((s, e))
    return merged


def print_ranges(lines: List[str], ranges: List[Tuple[int,int]], file=sys.stdout):
    for idx, (s, e) in enumerate(ranges, start=1):
        print(f"=== Match {idx}: lines {s+1}-{e+1} ===", file=file)
        for i in range(s, e+1):
            # show line numbers
            print(f"{i+1:6d}: {lines[i].rstrip()}", file=file)
        print(file=file)


def main():
    p = argparse.ArgumentParser(description="Extract context around matching lines")
    p.add_argument("file", nargs="?", default="log.txt", help="Path to the log file (default: log.txt)")
    p.add_argument("-p", "--pattern", default="[ERROR]", help="Substring or regex to match (default: '[ERROR]')")
    p.add_argument("-c", "--context", type=int, default=5, help="Number of context lines before and after each match (default: 5)")
    p.add_argument("-r", "--regex", action="store_true", help="Treat pattern as a regular expression")
    p.add_argument("-o", "--output", help="Output file path (default: stdout)")
    args = p.parse_args()

    try:
        with open(args.file, "r", encoding="utf-8", errors="replace") as fh:
            lines = fh.readlines()
    except FileNotFoundError:
        print(f"File not found: {args.file}", file=sys.stderr)
        sys.exit(2)

    ranges = find_match_ranges(lines, args.pattern, args.context, args.regex)
    
    if args.output:
        try:
            with open(args.output, "w", encoding="utf-8") as out_f:
                if not ranges:
                    print("No matches found.", file=out_f)
                else:
                    print_ranges(lines, ranges, file=out_f)
        except Exception as e:
            print(f"Error writing to output file: {e}", file=sys.stderr)
            sys.exit(1)
    else:
        if not ranges:
            print("No matches found.")
        else:
            print_ranges(lines, ranges)


if __name__ == "__main__":
    main()
