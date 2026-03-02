#!/usr/bin/env python3
"""
SQLite Schema Analyzer
Generates a comprehensive report of database schema including tables, columns, indexes, and relationships.

Usage:
    python analyze_schema.py <database_path> [--output report.md]

Example:
    python analyze_schema.py ~/AppData/Roaming/com.fritzprix.libragent/libragent_v2.db
    python analyze_schema.py data.db --output schema_report.md
"""

import sqlite3
import sys
import argparse
from pathlib import Path
from datetime import datetime


def analyze_schema(db_path: str, output_path: str = None):
    """Analyze SQLite database schema and generate report."""
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Get database file size
    db_size = Path(db_path).stat().st_size / (1024 * 1024)  # MB
    
    report_lines = [
        "# SQLite Database Schema Report",
        f"\n**Database**: `{db_path}`",
        f"**Size**: {db_size:.2f} MB",
        f"**Generated**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
        "\n---\n",
    ]
    
    # Get all tables
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
    tables = [row[0] for row in cursor.fetchall()]
    
    report_lines.append(f"## Overview\n")
    report_lines.append(f"**Total Tables**: {len(tables)}\n")
    
    # Analyze each table
    for table_name in tables:
        report_lines.append(f"\n## Table: `{table_name}`\n")
        
        # Row count
        cursor.execute(f"SELECT COUNT(*) FROM {table_name};")
        row_count = cursor.fetchone()[0]
        report_lines.append(f"**Row Count**: {row_count}\n")
        
        # Column information
        cursor.execute(f"PRAGMA table_info({table_name});")
        columns = cursor.fetchall()
        
        report_lines.append("\n### Columns\n")
        report_lines.append("| # | Name | Type | NOT NULL | Default | Primary Key |")
        report_lines.append("|---|------|------|----------|---------|-------------|")
        
        for col in columns:
            cid, name, type_, notnull, dflt_value, pk = col
            report_lines.append(
                f"| {cid} | `{name}` | {type_} | {'✓' if notnull else ''} | "
                f"{dflt_value if dflt_value else ''} | {'✓' if pk else ''} |"
            )
        
        # Foreign keys
        cursor.execute(f"PRAGMA foreign_key_list({table_name});")
        fkeys = cursor.fetchall()
        
        if fkeys:
            report_lines.append("\n### Foreign Keys\n")
            report_lines.append("| Column | References | On Update | On Delete |")
            report_lines.append("|--------|------------|-----------|-----------|")
            
            for fk in fkeys:
                id_, seq, table, from_, to, on_update, on_delete, match = fk
                report_lines.append(
                    f"| `{from_}` | `{table}.{to}` | {on_update} | {on_delete} |"
                )
        
        # Indexes
        cursor.execute(f"PRAGMA index_list({table_name});")
        indexes = cursor.fetchall()
        
        if indexes:
            report_lines.append("\n### Indexes\n")
            for idx in indexes:
                seq, name, unique, origin, partial = idx
                cursor.execute(f"PRAGMA index_info({name});")
                idx_cols = cursor.fetchall()
                col_names = [col[2] for col in idx_cols]
                
                unique_str = "UNIQUE " if unique else ""
                report_lines.append(f"- {unique_str}`{name}` on ({', '.join(col_names)})")
        
        # Sample data
        cursor.execute(f"SELECT * FROM {table_name} LIMIT 3;")
        samples = cursor.fetchall()
        
        if samples and row_count > 0:
            report_lines.append("\n### Sample Data (first 3 rows)\n")
            col_names = [desc[0] for desc in cursor.description]
            
            # Create markdown table
            report_lines.append("| " + " | ".join(col_names) + " |")
            report_lines.append("|" + "|".join(["---"] * len(col_names)) + "|")
            
            for row in samples:
                formatted_row = []
                for val in row:
                    if val is None:
                        formatted_row.append("NULL")
                    elif isinstance(val, str) and len(val) > 50:
                        formatted_row.append(f"{val[:47]}...")
                    else:
                        formatted_row.append(str(val))
                report_lines.append("| " + " | ".join(formatted_row) + " |")
    
    conn.close()
    
    # Write or print report
    report = "\n".join(report_lines)
    
    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(report)
        print(f"✅ Schema report generated: {output_path}")
    else:
        print(report)


def main():
    parser = argparse.ArgumentParser(description="Analyze SQLite database schema")
    parser.add_argument("database", help="Path to SQLite database file")
    parser.add_argument("--output", "-o", help="Output file path (optional)")
    
    args = parser.parse_args()
    
    if not Path(args.database).exists():
        print(f"❌ Error: Database file not found: {args.database}")
        sys.exit(1)
    
    try:
        analyze_schema(args.database, args.output)
    except Exception as e:
        print(f"❌ Error analyzing database: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
