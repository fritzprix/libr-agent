#!/usr/bin/env python3
"""
SQLite Orphaned Records Finder
Detects orphaned records where foreign key references point to non-existent records.

Usage:
    python find_orphans.py <database_path> [--table TABLE_NAME]

Example:
    python find_orphans.py ~/AppData/Roaming/com.fritzprix.libragent/libragent_v2.db
    python find_orphans.py data.db --table assistants
"""

import sqlite3
import sys
import argparse
from pathlib import Path
from typing import List, Dict, Tuple


def find_orphaned_records(db_path: str, target_table: str = None) -> Dict[str, List[Dict]]:
    """Find orphaned records in database tables."""
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Get all tables or just target table
    if target_table:
        tables = [target_table]
    else:
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
        tables = [row[0] for row in cursor.fetchall()]
    
    orphans_report = {}
    
    for table_name in tables:
        # Get foreign keys for this table
        cursor.execute(f"PRAGMA foreign_key_list({table_name});")
        fkeys = cursor.fetchall()
        
        if not fkeys:
            continue
        
        table_orphans = []
        
        for fk in fkeys:
            id_, seq, ref_table, from_col, to_col, on_update, on_delete, match = fk
            
            # Query to find orphaned records
            query = f"""
            SELECT t.rowid, t.{from_col}
            FROM {table_name} t
            LEFT JOIN {ref_table} r ON t.{from_col} = r.{to_col}
            WHERE t.{from_col} IS NOT NULL AND r.{to_col} IS NULL
            """
            
            try:
                cursor.execute(query)
                orphaned = cursor.fetchall()
                
                if orphaned:
                    table_orphans.append({
                        'foreign_key': f"{from_col} -> {ref_table}.{to_col}",
                        'count': len(orphaned),
                        'examples': orphaned[:5]  # First 5 examples
                    })
            except sqlite3.Error as e:
                print(f"⚠️ Warning: Could not check {table_name}.{from_col}: {e}")
        
        if table_orphans:
            orphans_report[table_name] = table_orphans
    
    conn.close()
    return orphans_report


def print_orphans_report(report: Dict[str, List[Dict]]):
    """Print formatted orphans report."""
    
    if not report:
        print("✅ No orphaned records found!")
        return
    
    print("# Orphaned Records Report\n")
    
    total_orphans = sum(
        sum(fk['count'] for fk in table_data)
        for table_data in report.values()
    )
    
    print(f"**Total Orphaned Records**: {total_orphans}\n")
    
    for table_name, fk_orphans in report.items():
        print(f"\n## Table: `{table_name}`\n")
        
        for fk_data in fk_orphans:
            print(f"### Foreign Key: `{fk_data['foreign_key']}`")
            print(f"**Orphaned Count**: {fk_data['count']}\n")
            
            if fk_data['examples']:
                print("**Sample Orphaned Records** (rowid, foreign_key_value):\n")
                for rowid, fk_val in fk_data['examples']:
                    print(f"- Row {rowid}: `{fk_val}`")
                print()


def main():
    parser = argparse.ArgumentParser(description="Find orphaned records in SQLite database")
    parser.add_argument("database", help="Path to SQLite database file")
    parser.add_argument("--table", "-t", help="Check only specific table (optional)")
    
    args = parser.parse_args()
    
    if not Path(args.database).exists():
        print(f"❌ Error: Database file not found: {args.database}")
        sys.exit(1)
    
    try:
        print(f"🔍 Analyzing database: {args.database}")
        if args.table:
            print(f"   Checking table: {args.table}")
        print()
        
        report = find_orphaned_records(args.database, args.table)
        print_orphans_report(report)
        
    except Exception as e:
        print(f"❌ Error analyzing database: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
