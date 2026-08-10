#!/usr/bin/env python3
"""One-shot data migration: SQLite -> PostgreSQL for LegalGenie Agent.

Usage:
    ./scripts/migrate_sqlite_to_pg.py \
        --sqlite data/legal_minds.db \
        --pg "postgres://legalgenie:legalgenie_dev@127.0.0.1:5432/legalgenie"

The target PostgreSQL must be empty (or already migrated) — the script imports
business tables only, never schema. It maps SQLite TEXT/INTEGER columns onto
the PostgreSQL TEXT/BIGINT schema produced by migrations_pg.

Safety: run against a copy first; the script does not drop or overwrite any
existing row unless --truncate is passed.
"""
import argparse
import sys

try:
    import psycopg2
    import psycopg2.extras
except ImportError:
    print("missing deps: pip install psycopg2-binary", file=sys.stderr)
    sys.exit(1)

# Business tables to migrate, in dependency order (FK-friendly).
TABLES = [
    "users",
    "roles",
    "user_roles",
    "cases",
    "case_members",
    "evidence_files",
    "event_nodes",
    "node_evidence_links",
    "persons",
    "person_case_links",
    "person_relationships",
    "evidence_file_chunks",
    "operation_logs",
    "search_histories",
    "user_hot_searches",
    "hot_searches",
    "export_records",
    "jobs",
    "idempotency_records",
]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--sqlite", required=True, help="path to source sqlite db")
    ap.add_argument("--pg", required=True, help="postgres dsn")
    ap.add_argument("--truncate", action="store_true", help="truncate pg tables first")
    args = ap.parse_args()

    import sqlite3

    src = sqlite3.connect(args.sqlite)
    src.row_factory = sqlite3.Row
    conn = psycopg2.connect(args.pg)
    conn.autocommit = False
    cur = conn.cursor()

    for table in TABLES:
        cols = [r["name"] for r in src.execute(f"PRAGMA table_info({table})")]
        if not cols:
            continue
        rows = src.execute(f"SELECT {', '.join(cols)} FROM {table}").fetchall()
        if not rows:
            continue

        if args.truncate:
            cur.execute(f'TRUNCATE TABLE "{table}" CASCADE')

        placeholders = ", ".join(["%s"] * len(cols))
        col_list = ", ".join(f'"{c}"' for c in cols)
        insert = f'INSERT INTO "{table}" ({col_list}) VALUES ({placeholders})'

        batch = [tuple(r[c] for c in cols) for r in rows]
        psycopg2.extras.execute_batch(cur, insert, batch, page_size=1000)
        print(f"{table}: {len(batch)} rows")

    conn.commit()
    print("done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
