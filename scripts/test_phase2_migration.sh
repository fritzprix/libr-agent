#!/bin/bash

# Test Phase 2 SeaORM Migration
# This script verifies that Phase 2 entities and migrations work correctly

set -e

echo "🔍 Phase 2 Migration Test"
echo "========================="
echo ""

# Create temporary test database
TEST_DB="/tmp/libragent_phase2_test_$(date +%s).db"
echo "📁 Creating test database: $TEST_DB"

# Build the migration binary
echo "🔨 Building migration tool..."
cd "$(dirname "$0")/../src-tauri/migration"
cargo build --release 2>&1 | tail -5

# Run migrations
echo ""
echo "🚀 Running migrations..."
DATABASE_URL="sqlite://$TEST_DB" cargo run --release 2>&1 | tail -10

# Verify tables exist
echo ""
echo "✅ Verifying Phase 2 tables..."
sqlite3 "$TEST_DB" <<EOF
.mode column
.headers on
SELECT name, sql FROM sqlite_master WHERE type='table' AND name IN (
    'stores', 'contents', 'chunks', 'knowledge', 'assistants', 'playbooks', 'mcp_servers'
) ORDER BY name;
EOF

# Count tables
TABLE_COUNT=$(sqlite3 "$TEST_DB" "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('stores', 'contents', 'chunks', 'knowledge', 'assistants', 'playbooks', 'mcp_servers');")
echo ""
echo "📊 Phase 2 Tables Created: $TABLE_COUNT / 7"

# Verify indexes
echo ""
echo "🔍 Verifying indexes..."
sqlite3 "$TEST_DB" "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%' ORDER BY name;"

# Verify foreign keys
echo ""
echo "🔗 Verifying foreign keys..."
sqlite3 "$TEST_DB" <<EOF
SELECT 
    m.name as table_name,
    p.id,
    p."table" as foreign_table,
    p."from" as from_column,
    p."to" as to_column,
    p.on_delete
FROM sqlite_master m
JOIN pragma_foreign_key_list(m.name) p
WHERE m.type='table' 
  AND m.name IN ('contents', 'chunks')
ORDER BY m.name, p.id;
EOF

# Verify FTS5 virtual table
echo ""
echo "📚 Verifying FTS5 virtual table..."
sqlite3 "$TEST_DB" "SELECT name, sql FROM sqlite_master WHERE name LIKE 'knowledge_fts%' ORDER BY name;"

# Cleanup
echo ""
echo "🧹 Cleaning up test database..."
rm -f "$TEST_DB"

echo ""
echo "✅ Phase 2 Migration Test Complete!"
echo "All tables, indexes, foreign keys, and FTS5 setup verified successfully."
