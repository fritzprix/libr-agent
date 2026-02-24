#!/usr/bin/env python3
"""
SQLite Sample Data Exporter
Exports sample data from database tables in various formats for debugging and analysis.

Usage:
    python export_sample.py <database_path> <table_name> [--format FORMAT] [--limit N]

Example:
    python export_sample.py data.db mcp_servers --format json --limit 5
    python export_sample.py data.db assistants --format csv
    python export_sample.py data.db sessions --format markdown --limit 10
"""

import sqlite3
import sys
import argparse
import json
import csv
from pathlib import Path
from typing import List, Dict, Any


def export_as_json(rows: List[tuple], columns: List[str], output_file: str = None):
    """Export data as JSON."""
    data = [dict(zip(columns, row)) for row in rows]
    
    json_str = json.dumps(data, indent=2, default=str, ensure_ascii=False)
    
    if output_file:
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(json_str)
        print(f"✅ Exported to: {output_file}")
    else:
        print(json_str)


def export_as_csv(rows: List[tuple], columns: List[str], output_file: str = None):
    """Export data as CSV."""
    if output_file:
        with open(output_file, 'w', newline='', encoding='utf-8') as f:
            writer = csv.writer(f)
            writer.writerow(columns)
            writer.writerows(rows)
        print(f"✅ Exported to: {output_file}")
    else:
        writer = csv.writer(sys.stdout)
        writer.writerow(columns)
        writer.writerows(rows)


def export_as_markdown(rows: List[tuple], columns: List[str], output_file: str = None):
    """Export data as Markdown table."""
    lines = []
    
    # Header
    lines.append("| " + " | ".join(columns) + " |")
    lines.append("|" + "|".join(["---"] * len(columns)) + "|")
    
    # Data rows
    for row in rows:
        formatted_row = []
        for val in row:
            if val is None:
                formatted_row.append("NULL")
            elif isinstance(val, str) and len(val) > 100:
                formatted_row.append(f"{val[:97]}...")
            else:
                formatted_row.append(str(val).replace("|", "\\|"))
        lines.append("| " + " | ".join(formatted_row) + " |")
    
    output = "\n".join(lines)
    
    if output_file:
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(output)
        print(f"✅ Exported to: {output_file}")
    else:
        print(output)


def export_as_sql(table_name: str, rows: List[tuple], columns: List[str], output_file: str = None):
    """Export data as SQL INSERT statements."""
    lines = []
    
    for row in rows:
        values = []
        for val in row:
            if val is None:
                values.append("NULL")
            elif isinstance(val, str):
                # Escape single quotes
                escaped = val.replace("'", "''")
                values.append(f"'{escaped}'")
            else:
                values.append(str(val))
        
        sql = f"INSERT INTO {table_name} ({', '.join(columns)}) VALUES ({', '.join(values)});"
        lines.append(sql)
    
    output = "\n".join(lines)
    
    if output_file:
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(output)
        print(f"✅ Exported to: {output_file}")
    else:
        print(output)


def export_sample_data(
    db_path: str,
    table_name: str,
    format: str = 'json',
    limit: int = 10,
    output_file: str = None
):
    """Export sample data from table."""
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Verify table exists
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name=?;", (table_name,))
    if not cursor.fetchone():
        print(f"❌ Error: Table '{table_name}' not found")
        conn.close()
        sys.exit(1)
    
    # Get data
    cursor.execute(f"SELECT * FROM {table_name} LIMIT ?;", (limit,))
    rows = cursor.fetchall()
    columns = [desc[0] for desc in cursor.description]
    
    conn.close()
    
    if not rows:
        print(f"⚠️ Warning: Table '{table_name}' is empty")
        return
    
    # Export in requested format
    if format == 'json':
        export_as_json(rows, columns, output_file)
    elif format == 'csv':
        export_as_csv(rows, columns, output_file)
    elif format == 'markdown':
        export_as_markdown(rows, columns, output_file)
    elif format == 'sql':
        export_as_sql(table_name, rows, columns, output_file)
    else:
        print(f"❌ Error: Unsupported format '{format}'")
        sys.exit(1)
    
    print(f"\n📊 Exported {len(rows)} rows from table '{table_name}'")


def main():
    parser = argparse.ArgumentParser(description="Export sample data from SQLite database")
    parser.add_argument("database", help="Path to SQLite database file")
    parser.add_argument("table", help="Table name to export")
    parser.add_argument(
        "--format", "-f",
        choices=['json', 'csv', 'markdown', 'sql'],
        default='json',
        help="Export format (default: json)"
    )
    parser.add_argument(
        "--limit", "-l",
        type=int,
        default=10,
        help="Number of rows to export (default: 10)"
    )
    parser.add_argument(
        "--output", "-o",
        help="Output file path (optional, prints to stdout if not provided)"
    )
    
    args = parser.parse_args()
    
    if not Path(args.database).exists():
        print(f"❌ Error: Database file not found: {args.database}")
        sys.exit(1)
    
    try:
        export_sample_data(
            args.database,
            args.table,
            args.format,
            args.limit,
            args.output
        )
    except Exception as e:
        print(f"❌ Error exporting data: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
