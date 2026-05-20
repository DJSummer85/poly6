import sqlite3
import urllib.request
import json
import math

db_path = 'backend/data/polymarket_v2.db'
conn = sqlite3.connect(db_path)
c = conn.cursor()

# Fetch running bots
c.execute("SELECT id, name, strategy_type, market_id, bet_size, kelly_fraction FROM bot_configs WHERE status='running'")
bots = c.fetchall()

print(f"Loaded {len(bots)} running bots.")

# Fetch active markets from API
try:
    res = json.loads(urllib.request.urlopen('http://localhost:3001/api/market/active?timeframe=5').read().decode())
    active_markets = res['markets']
except Exception as e:
    print("Failed to fetch active markets:", e)
    active_markets = []

# Map market by asset
markets_by_asset = {m['asset'].upper(): m for m in active_markets}

print(f"Active markets: {list(markets_by_asset.keys())}")

def calculate_7_factor_confidence(yes_price, no_price, side):
    # Simplified simulation of 7-factor confidence
    confidence = 0.50
    
    # Odds disagreement penalty (reversed bug check)
    # Original buggy code:
    # odds_direction = 1.0 - yes_price if side == "YES" else yes_price
    # odds_disagreement = 1.0 - odds_direction
    # confidence -= min(odds_disagreement * 0.08, 0.08)
    
    odds_direction = 1.0 - yes_price if side == "YES" else yes_price
    odds_disagreement = 1.0 - odds_direction
    penalty = min(odds_disagreement * 0.08, 0.08)
    confidence -= penalty
    
    # Volatility penalty (low vol = 0.02)
    confidence -= 0.02
    
    return confidence

def calculate_bayesian_ev(raw_confidence, cost, fee_rate=0.02):
    p_win = raw_confidence
    p_lose = 1.0 - p_win
    net_profit_if_win = (1.0 - cost) * (1.0 - fee_rate)
    loss_if_lose = cost
    expected_value = (p_win * net_profit_if_win) - (p_lose * loss_if_lose)
    return expected_value

print("\n--- Diagnostic Pipeline Simulation ---")
for bot_id, name, strat_type, asset, bet_size, kelly in bots:
    asset_upper = asset.upper()
    if asset_upper == "AUTO":
        asset_upper = "BTC"
    
    market = markets_by_asset.get(asset_upper)
    if not market:
        print(f"Bot {name} ({asset_upper}): No active market found.")
        continue
        
    yes_price = market['yes_price']
    no_price = market['no_price']
    
    # Let's check both YES and NO directions
    for side in ["YES", "NO"]:
        cost = yes_price if side == "YES" else no_price
        
        # Strategy raw confidence
        strat_conf = 0.60 # Typical strategy confidence
        
        # 7-factor conf
        seven_factor = calculate_7_factor_confidence(yes_price, no_price, side)
        
        # Blended
        blended = strat_conf * 0.4 + seven_factor * 0.6
        
        # Expected value
        ev_with_fee = calculate_bayesian_ev(blended, cost, fee_rate=0.02)
        ev_no_fee = calculate_bayesian_ev(blended, cost, fee_rate=0.0)
        
        print(f"Bot: {name:<18} | Side: {side} | Cost: {cost:.3f} | 7-Factor: {seven_factor:.3f} | Blended: {blended:.3f} | EV (with fee): {ev_with_fee:+.4f} | EV (no fee): {ev_no_fee:+.4f}")

conn.close()
