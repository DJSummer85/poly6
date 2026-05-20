import sqlite3

def check_decisions():
    db_path = "backend/data/polymarket_v2.db"
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    cursor = conn.cursor()
    
    query = """
    SELECT 
        d.created_at,
        b.name as bot_name,
        d.market_slug,
        d.outcome,
        d.signal_confidence,
        d.btc_price,
        d.btc_change,
        d.market_yes_price,
        d.market_no_price,
        d.time_remaining,
        d.decision_reason,
        d.executed,
        d.pnl
    FROM trade_decisions d
    JOIN bot_configs b ON d.bot_id = b.id
    ORDER BY d.created_at DESC
    LIMIT 50;
    """
    try:
        cursor.execute(query)
        rows = cursor.fetchall()
        print(f"Retrieved {len(rows)} recent decisions:")
        print(f"{'Time':<19} | {'Strategy':<16} | {'Outcome':<7} | {'Conf':<4} | {'BTC px':<8} | {'Chg %':<7} | {'YES':<4} | {'NO':<4} | {'Reason':<40} | {'Exec':<4}")
        print("-" * 125)
        for r in rows:
            chg_str = f"{r[6]*100:.4f}%" if r[6] is not None else "N/A"
            reason = r[10] if r[10] is not None else "N/A"
            print(f"{r[0][11:19]:<19} | {r[1]:<16} | {r[3]:<7} | {r[4]:>4.2f} | {r[5]:>8.1f} | {chg_str:<7} | {r[7]:>4.2f} | {r[8]:>4.2f} | {reason[:40]:<40} | {r[11]:<4}")
            
    except Exception as e:
        print("Error:", e)
    conn.close()

if __name__ == "__main__":
    check_decisions()
