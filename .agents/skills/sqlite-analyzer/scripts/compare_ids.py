#!/usr/bin/env python3
"""
SQLite ID Format Comparator
Detects potential bugs where entity names are used as IDs instead of actual ID values.
This was the bug pattern found in MCPServerDto mapping (model.name used instead of model.id).

Usage:
    python compare_ids.py <database_path> [--suspect-table TABLE_NAME]

Example:
    python compare_ids.py ~/AppData/Roaming/com.fritzprix.libragent/libragent_v2.db
    python compare_ids.py data.db --suspect-table mcp_servers
"""

import sqlite3
import sys
import argparse
import re
from pathlib import Path
from typing import List, Dict, Tuple


# Common ID formats
ID_PATTERNS = {
    'uuid_v4': re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$', re.I),
    'cuid': re.compile(r'^c[0-9a-z]{24}$'),
    'cuid2': re.compile(r'^[0-9a-z]{24}$'),  # CUID2 format (e.g., hiwqx3dj3tn82vt9amjysalj)
    'nanoid': re.compile(r'^[A-Za-z0-9_-]{21}$'),
    'numeric': re.compile(r'^\d+$'),
    'simple_string': re.compile(r'^[a-z][a-z0-9_-]*$', re.I),  # Human-readable names
}


def detect_id_format(value: str) -> str:
    """Detect ID format type."""
    if value is None:
        return 'null'
    
    value = str(value).strip()
    
    for format_name, pattern in ID_PATTERNS.items():
        if pattern.match(value):
            return format_name
    
    return 'unknown'


def analyze_table_ids(db_path: str, table_name: str) -> Dict:
    """Analyze ID columns in a table and detect potential name-as-id bugs."""
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Get table info
    cursor.execute(f"PRAGMA table_info({table_name});")
    columns = cursor.fetchall()
    
    # Find id and name columns
    id_col = None
    name_col = None
    
    for col in columns:
        cid, name, type_, notnull, dflt_value, pk = col
        if name.lower() in ['id', '_id', 'rowid']:
            id_col = name
        elif name.lower() in ['name', 'title', 'label']:
            name_col = name
    
    if not id_col:
        return {'error': f"No ID column found in table {table_name}"}
    
    # Analyze ID formats
    cursor.execute(f"SELECT {id_col}, {name_col if name_col else 'NULL'} FROM {table_name} LIMIT 100;")
    rows = cursor.fetchall()
    
    id_formats = {}
    name_formats = {}
    potential_bugs = []
    
    for row in rows:
        id_val = row[0]
        name_val = row[1] if name_col else None
        
        id_fmt = detect_id_format(id_val)
        id_formats[id_fmt] = id_formats.get(id_fmt, 0) + 1
        
        if name_val:
            name_fmt = detect_id_format(name_val)
            name_formats[name_fmt] = name_formats.get(name_fmt, 0) + 1
            
            # Bug detection: If ID looks like a name and name looks like an ID
            if id_fmt == 'simple_string' and name_fmt in ['cuid', 'cuid2', 'uuid_v4']:
                potential_bugs.append({
                    'id': id_val,
                    'name': name_val,
                    'id_format': id_fmt,
                    'name_format': name_fmt,
                    'suspicion': 'HIGH - ID column contains human-readable name, name column contains CUID/UUID'
                })
            
            # Also check if ID and name are swapped (same value)
            if id_val == name_val:
                potential_bugs.append({
                    'id': id_val,
                    'name': name_val,
                    'id_format': id_fmt,
                    'name_format': name_fmt,
                    'suspicion': 'MEDIUM - ID and name have identical values'
                })
    
    conn.close()
    
    return {
        'table': table_name,
        'id_column': id_col,
        'name_column': name_col,
        'row_count': len(rows),
        'id_formats': id_formats,
        'name_formats': name_formats,
        'potential_bugs': potential_bugs
    }


def find_all_id_mismatches(db_path: str, suspect_table: str = None) -> List[Dict]:
    """Scan database for potential ID format mismatches."""
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Get tables to check
    if suspect_table:
        tables = [suspect_table]
    else:
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
        tables = [row[0] for row in cursor.fetchall()]
    
    conn.close()
    
    results = []
    
    for table in tables:
        analysis = analyze_table_ids(db_path, table)
        if 'error' not in analysis and analysis.get('potential_bugs'):
            results.append(analysis)
    
    return results


def print_analysis_report(results: List[Dict]):
    """Print formatted analysis report."""
    
    if not results:
        print("✅ No ID format mismatches detected!")
        return
    
    print("# ID Format Mismatch Analysis\n")
    
    total_bugs = sum(len(r['potential_bugs']) for r in results)
    print(f"**Tables with Potential Issues**: {len(results)}")
    print(f"**Total Potential Bugs**: {total_bugs}\n")
    
    for result in results:
        print(f"\n## Table: `{result['table']}`\n")
        print(f"- **ID Column**: `{result['id_column']}`")
        print(f"- **Name Column**: `{result['name_column']}`")
        print(f"- **Rows Analyzed**: {result['row_count']}\n")
        
        print("### ID Format Distribution\n")
        for fmt, count in result['id_formats'].items():
            print(f"- {fmt}: {count} rows")
        
        if result['name_formats']:
            print("\n### Name Format Distribution\n")
            for fmt, count in result['name_formats'].items():
                print(f"- {fmt}: {count} rows")
        
        print("\n### Potential Bugs\n")
        for bug in result['potential_bugs']:
            print(f"#### {bug['suspicion']}\n")
            print(f"- **ID Value**: `{bug['id']}` (format: {bug['id_format']})")
            print(f"- **Name Value**: `{bug['name']}` (format: {bug['name_format']})")
            print()


def main():
    parser = argparse.ArgumentParser(
        description="Detect ID/name swapping bugs in SQLite database"
    )
    parser.add_argument("database", help="Path to SQLite database file")
    parser.add_argument("--suspect-table", "-t", help="Check only specific table (optional)")
    
    args = parser.parse_args()
    
    if not Path(args.database).exists():
        print(f"❌ Error: Database file not found: {args.database}")
        sys.exit(1)
    
    try:
        print(f"🔍 Analyzing ID formats in: {args.database}")
        if args.suspect_table:
            print(f"   Checking table: {args.suspect_table}")
        print()
        
        results = find_all_id_mismatches(args.database, args.suspect_table)
        print_analysis_report(results)
        
        if results:
            print("\n⚠️ **Action Required**: Review potential bugs above.")
            print("   Common fix: Check DTO/entity mapping - ensure `model.id` is used, not `model.name`")
        
    except Exception as e:
        print(f"❌ Error analyzing database: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
