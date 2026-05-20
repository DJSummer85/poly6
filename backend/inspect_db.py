import sqlite3
import os

db_path = 'data/polymarket_v2.db'
if not os.path.exists(db_path):
    # Try parent directory
    db_path = '../data/polymarket_v2.db'

print(f"Connecting to database at: {os.path.abspath(db_path)}")
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

# Get tables
cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
tables = [row[0] for row in cursor.fetchall()]
print("Tables:", tables)

for table in tables:
    cursor.execute(f"SELECT COUNT(*) FROM {table}")
    count = cursor.fetchone()[0]
    print(f"  {table}: {count} rows")

print("\n--- Recent Bot Logs ---")
try:
    cursor.execute("SELECT bot_id, level, message, created_at FROM bot_logs ORDER BY id DESC LIMIT 15")
    for r in cursor.fetchall():
        print(r)
except Exception as e:
    print("Could not query bot_logs:", e)

print("\n--- Recent Bot Configs (Running ones) ---")
try:
    cursor.execute("SELECT id, name, strategy_type, status, trading_mode, bet_size FROM bot_configs WHERE status='running'")
    for r in cursor.fetchall():
        print(r)
except Exception as e:
    print("Could not query bot_configs:", e)

conn.close()
