# Polymarket BTC 5-Min Profitabil Strategy

## Kutatási Összefoglaló

Források:
- `oasisnoehub/PolymarketBtcBot` — Oracle Lag + 11-indicator system
- `Naeaerc20/Polymarket-Multi-Strategy-Bot` — YES+NO Arbitrage
- `fzheng/polybot` — Two-leg arbitrage

---

## 1. YES+NO Arbitrage (GARANTÁLT PROFIT)

### Mechanizmus
```
Ha: UP_ár + DOWN_ár < $1.00
Akkor: Garantált profit = $1.00 - (UP_ár + DOWN_ár)
```

### Példa
```
UP = $0.49, DOWN = $0.49
Költség: $0.98
Kifizetés: $1.00 (mikorartioned semleges)
Profit: +$0.02 / ciklus
```

### Loss Prevention Mode (ajánlott)
```
1. Fázis: WATCH TRIGGER_RANGE (pl. 0.52-0.54)
   → Buy whichever side hits this first
   
2. Fázis: Wait for opposite side to hit PRICE_RANGE (pl. 0.40-0.45)
   → Then buy opposite side

Ez megakadályozza hogy az egyik oldal folyamatosan emelkedjen és soha ne essen vissza.
```

### Paraméterek
```
BTC_PRICE_RANGE=0.40-0.45     # Standard buy range (both sides)
BTC_TRIGGER_RANGE=0.52-0.54   # First-side trigger (LP mode)
BTC_AMOUNT_TO_BUY=$2.50       # Per side
BTC_LOSS_PREVENTION=true      # Recommended
```

---

## 2. Oracle Lag Arbitrage

### Mechanizmus
```
T+0:  Chainlink leolvassa: BTC = $84,200 (window open price LOCKED)
T+28s: BTC = $84,326 (+0.15%) Binance/OKX/Kraken — mind egyetért
T+30s: Chainlink még mindig $84,200 (késés = 30 másodperc)
T+31s: Bot vásárol UP-t $0.54-ért (piac alulárazza az edge-t)
T+34s: Chainlink frissül $84,326-ra
T+300s: Window záródik, BTC = $84,410 → UP nyert
        Profit: ($1.00 - $0.54) × shares = ~85% return
```

### 5-Dimenziós Confidence Score
| Dimenzió | Súly | Logika |
|----------|------|--------|
| Lag Duration | 30% | Ideal: 10-35s |
| Price Divergence | 30% | Ideal: 0.08-0.35% |
| Cross-Exchange Agreement | 20% | Binance+OKX+Kraken agree |
| Tick Momentum | 12% | Last 10s trades confirm direction |
| Historical Accuracy | 8% | Rolling 50-signal win rate |

### Confidence → Win Rate Mapping
| Confidence | Win Rate | Action |
|------------|----------|--------|
| ≥ 0.85 | ~72% | Full Kelly |
| ≥ 0.75 | ~65% | Strong bet |
| ≥ 0.65 | ~59% | Normal bet |
| ≥ 0.50 | ~54% | Minimum bet |
| < 0.50 | < 54% | Skip |

---

## 3. Window Delta (King Strategy)

### Mechanizmus
- BTC 5 perces window-on belüli ármozgás → közvetlen piaci válasz
- 8x súlyozású a 11-indikátor rendszerben
- Minél nagyobb a delta, annál biztosabb az irány

### Delta küszöbök
```
STRONG:   ≥ 0.25%  → weight 8.0
MEDIUM:   ≥ 0.15%  → weight 5.0
WEAK:     ≥ 0.08%  → weight 3.0
NOISE:    ≥ 0.03%  → weight 1.0
FLAT:     <  0.03% → weight 0.0
```

---

## 4. Kelly Criterion Bet Sizing

### Formula
```
f* = (p × b - q) / b
ahol:
  p = win probability (from confidence calibration)
  q = 1 - p
  b = (1 - token_price) / token_price  (payout ratio)
```

### Frakcionális Kelly
```
Fractional Kelly = 40% of full Kelly (conservative)
Mode multiplier: ultra = 1.10×
```

### Streak adjustments
| Streak | Kelly Multiplier |
|--------|------------------|
| 5+ wins | +40% |
| 4 wins | +30% |
| 3 wins | +20% |
| 2 wins | +10% |
| 2 losses | -15% |
| 3 losses | -30% |
| 4+ losses | -60% |

---

## 5. Volatility Regime Detection

ATR (Average True Range) alapján:
```
ATR < 15      → Quiet    → bet × 0.40
ATR 15-35     → Low      → bet × 0.70
ATR 35-350    → Ideal    → bet × 1.00 (default)
ATR 350-800   → Volatile → bet × 0.75
ATR > 800     → Extreme  → bet × 0.55
```

---

## 6. Risk Management

### Hard Stops
| Condition | Trigger | Action |
|-----------|---------|--------|
| Peak drawdown | > 55% from ATH | Halt |
| Consecutive losses | 5 in a row | Halt |
| Daily loss | ≥ 28% of bankroll | Halt |
| Min bankroll | < $1 | Exit |

### Profit Lock
```
Trigger: bankroll ≥ 140% of original
Lock: 35% of profits reserved
Effect: future bets capped at 20% of bankroll
```

---

## Javasolt Stratégia Stack

### Tier 1: YES+NO Arbitrage (alap, folyamatos)
- Watch: UP + DOWN combined < $0.98
- Bet: Equal amounts on both sides
- Profit: Guaranteed $0.02+ per cycle
- Risk: Very low

### Tier 2: Oracle Lag (high confidence, ritka)
- Trigger: Lag > 10s AND divergence > 0.08% AND 3 exchanges agree
- Confidence ≥ 0.65 required
- Kelly fraction: 25-40% of bankroll

### Tier 3: Window Delta Snipe (last 50 seconds)
- Trigger: delta ≥ 0.15% in last 5 minutes
- Only trade if ATR regime is "ideal" or "low"
- Confidence ≥ 0.55 required

---

## Paraméterek kalibráció

### Saját Bot整改
```rust
// strategies.rs - Javasolt módosítások:

// 1. YES+NO Arbitrage threshold
const YES_NO_COMBINED_MAX: f64 = 0.97;  // Ha UP+DOWN < 0.97, arbitrage!

// 2. Oracle Lag küszöbök (jelenleg 0.015 = túl magas)
const MIN_DELTA: f64 = 0.0008;   // 0.08% minimum (weak signal)
const IDEAL_DELTA_MIN: f64 = 0.0015;  // 0.15% (medium signal)
const IDEAL_DELTA_MAX: f64 = 0.0035;   // 0.35% (strong signal)

// 3. Kelly fraction
const KELLY_FRACTION: f64 = 0.40;  // Conservative fractional Kelly
const MIN_EDGE: f64 = 0.02;        // Minimum 2% positive edge required
```

---

## Tesztelési Terv

### Fázis 1: Paper trading (1-2 hét)
- 3 bot, különböző stratégiákkal
- $10-10 kezdőbalance
- Cél: Verifikálni hogy a stratégiák működnek

### Fázis 2: Mini live ($25/bot)
- Kis valós pénz
- 2-3 profitábilis bot kiválasztása
- Cél: $1-2 / óra / bot

### Fázis 3: Scale up
- Max $100/bot
- Automatikus Kelly sizing
- Cél: $3-4 / óra / bot

---

## Profit cél

| Bot | Hourly Target | Daily Target (8h) |
|-----|--------------|-------------------|
| 1 bot | $3-4 | $24-32 |
| 3 bots | $9-12 | $72-96 |

$3-4 / hour / bot × 3 bots = $9-12 / hour total
