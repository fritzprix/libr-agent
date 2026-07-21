---
description: Run and verify database migrations
---

Run and verify SeaORM database migrations for LibrAgent.

1. Run migrations: `pnpm db:migrate` or equivalent SeaORM migration command
2. Verify schema: Use `scripts/verify_phase2_migration.sh` or `scripts/test_planning_module.sh` patterns
3. Validate against in-memory SQLite using `Migrator::up()` with `SqlitePoolOptions` + `SqlxSqliteConnector`

Database uses SeaORM + SQLite (`libsqlite3-sys` bundled). Migration correctness is critical for schema integrity.
