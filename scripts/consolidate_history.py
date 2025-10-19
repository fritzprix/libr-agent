#!/usr/bin/env python3
"""
Consolidate History Files Script

This script scans the /docs/history folder for .md files that are 3 or more days old
(based on filename date pattern YYYYMMDD), and consolidates them into a single markdown file.

Usage: python scripts/consolidate_history.py
"""

import os
import re
import tarfile
import shutil
from datetime import datetime, timedelta
from typing import Optional

# Configuration
HISTORY_DIR = "/home/fritzprix/my_works/tauri-agent/docs/history"
OUTPUT_FILE = os.path.join(HISTORY_DIR, "consolidated_history_20250906.md")
ARCHIVE_DIR = os.path.join(HISTORY_DIR, "archive")
DAYS_OLD = 3

def parse_date_from_filename(filename: str) -> Optional[datetime.date]:
    """
    Extract date from filename using YYYYMMDD pattern.
    Example: 'debug_20250902_0007.md' -> 2025-09-02
    """
    match = re.search(r'(\d{8})', filename)
    if not match:
        return None
    date_str = match.group(1)
    try:
        return datetime.strptime(date_str, '%Y%m%d').date()
    except ValueError:
        return None

def is_older_than_days(file_date: datetime.date, days: int) -> bool:
    """
    Check if file_date is older than the specified number of days from today.
    """
    today = datetime.now().date()
    cutoff = today - timedelta(days=days)
    return file_date < cutoff

def consolidate_history():
    """
    Main function to consolidate old history files.
    """
    if not os.path.exists(HISTORY_DIR):
        print(f"Error: Directory {HISTORY_DIR} does not exist.")
        return

    # Get all .md files
    files = [f for f in os.listdir(HISTORY_DIR) if f.endswith('.md')]
    selected_files = []

    for filename in files:
        file_date = parse_date_from_filename(filename)
        if file_date and is_older_than_days(file_date, DAYS_OLD):
            selected_files.append(filename)

    if not selected_files:
        print("No files older than 3 days found.")
        return

    # Read and concatenate files
    content = ""
    for filename in selected_files:
        filepath = os.path.join(HISTORY_DIR, filename)
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                file_content = f.read()
            content += f"# {filename}\n\n{file_content}\n\n---\n\n"
        except Exception as e:
            print(f"Error reading {filename}: {e}")

    # Write output file
    try:
        with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"Consolidated {len(selected_files)} files into {OUTPUT_FILE}")
    except Exception as e:
        print(f"Error writing output file: {e}")

    # Create tar.gz archive
    tar_file = OUTPUT_FILE.replace('.md', '.tar.gz')
    try:
        with tarfile.open(tar_file, 'w:gz') as tar:
            tar.add(OUTPUT_FILE, arcname=os.path.basename(OUTPUT_FILE))
        print(f"Created tar archive: {tar_file}")

        # Create archive directory if it doesn't exist
        os.makedirs(ARCHIVE_DIR, exist_ok=True)

        # Copy tar file to archive
        archive_tar = os.path.join(ARCHIVE_DIR, os.path.basename(tar_file))
        shutil.copy2(tar_file, archive_tar)
        print(f"Copied to archive: {archive_tar}")

        # Delete input files only (keep consolidated .md)
        for filename in selected_files:
            filepath = os.path.join(HISTORY_DIR, filename)
            os.remove(filepath)
            print(f"Deleted input file: {filename}")

    except Exception as e:
        print(f"Error creating tar archive: {e}")

if __name__ == "__main__":
    consolidate_history()
