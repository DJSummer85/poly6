# Demo/Live Mode es Frontend Atalakitas Terv

Datum: 2026-05-05

## Cel

Egy olyan Polymarket trading bot rendszer ujratervezese, ahol a demo mode megbizhatoan, determinisztikusan es merhetoen teszteli a strategiakat, a live mode pedig ugyanarra a dontesi logikara epul, de kulon, biztonsagos order execution retegen keresztul kereskedik. A frontend celja nem "latvanyos dashboard", hanem operatori felulet: gyorsan olvashato allapot, tiszta demo/live elvalasztas, ertheto strategia-teszteles, es azonnali hibadiagnosztika.

## Jelenlegi helyzet rovid diagnosztikaja

- A demo es live fogalmak keverednek: frontendben `demo/live`, backendben foleg `paper/live`, a bulk run demo balansza `$100`, az egyedi start `$10`.
- A backendben tobb vegrehajtasi utvonal letezik: `BotOrchestrator` es `BotExecutor`. Nem egyertelmu, melyik a kanonikus bot engine.
- Az aktualis live order ag az `orchestrator.rs`-ben nem tenyleges Polymarket ordert ad fel, hanem szimulalt order id-t ad vissza.
- A paper settlement csak egy pending bet modellre epul, ami egyszerre egy poziciot kezel, es nem modellezi eleg pontosan az order lifecycle-t, fillt, piacvaltast, settlementet es PnL-t.
- A strategia teszteles nem kulon termekfunkcio: nincs izolalt strategy lab, nincs visszajatszhato market adat, nincs stabil run/report modell.
- Az SSE market adat es a bot event adat egy streamen logikailag keveredik, es nincs explicit frontend event contract verzio.
- A frontend sok collapsible panelbol all; ez rendezettnek tunhet kod szinten, de operatori szempontbol szetaprozott, nehez priorizalni, hogy most mire kell nezni.
- A design tul sok dekorativ hatast es kartyat hasznal egy trading toolhoz kepest. A fontos allapotok, hibak, modok es kovetkezo teendok nem eleg hierarchikusak.

## Alapelv

A demo es live mode kozott nem a strategia, hanem az execution adapter kulonbozzon.

Kozos marad:
- market data normalizalas
- strategy input context
- strategy evaluation
- risk check
- event journal
- metrika-szamitas

Kulonbozik:
- demo: szimulalt order, determinisztikus fill, lokalis settlement, nincs credential igeny
- live: Polymarket CLOB order, API credential, allowance/balance/gas check, valos order status sync

## Celallapot architektura

### Backend retegek

1. `MarketDataService`
   - Felelos az aktiv BTC 5m piac felfedezeseert, token id-kert, oddsokert, BTC arert es price-to-beat ertekert.
   - Egyetlen kanonikus market snapshot strukturat ad vissza.
   - Nem a frontend es nem a bot engine talalja ki kulon-kulon, hogy mi az aktualis market.

2. `StrategyEngine`
   - Tiszta, mellekhatasmentes strategia evaluation.
   - Input: normalizalt market snapshot, BTC history/window, bot config, risk context.
   - Output: `Hold` vagy `TradeIntent`, teljes magyarazattal.
   - Ugyanez fut demo, replay/backtest es live modban.

3. `ExecutionEngine`
   - Kozos pipeline: validate intent -> risk check -> execute -> journal -> emit event.
   - Ket adapterrel:
     - `PaperExecutionAdapter`
     - `LivePolymarketExecutionAdapter`

4. `SettlementService`
   - Demo modban a market zarasakor lezari a szimulalt poziciokat.
   - Live modban a Polymarket poziciok/order status alapjan syncel.
   - A PnL, win/loss, ROI es drawdown szamitasa innen induljon, ne UI oldali kovetkeztetesbol.

5. `EventJournal`
   - Minden bot run es strategy teszt visszakeresheto event listat kap:
     - market snapshot received
     - strategy evaluated
     - risk rejected
     - order submitted
     - order filled/simulated
     - market settled
     - PnL updated
   - Az SSE csak ezt az allapotot tovabbitja, nem ez legyen az uzleti igazsag forrasa.

### Javasolt backend fajlstruktura

- `backend/src/trading/market_data.rs`: normalizalt market snapshot, BTC price source, active market discovery.
- `backend/src/trading/strategy_engine.rs`: strategia input/output contract es evaluation wrapper.
- `backend/src/trading/execution/mod.rs`: kozos execution interface.
- `backend/src/trading/execution/paper.rs`: demo order es settlement.
- `backend/src/trading/execution/live.rs`: Polymarket CLOB execution.
- `backend/src/trading/event_journal.rs`: bot/run/event naplozas.
- `backend/src/trading/run_manager.rs`: a jelenlegi orchestrator egyszerusitett utodja.
- `backend/src/api/strategy_tests.rs`: demo strategy lab API.
- `backend/src/api/trading_modes.rs`: mode status, readiness, live preflight.
- `backend/src/api/sse.rs`: csak stream contract es event tovabbitas, kevesebb uzleti logika.

Meglevo fajlok, amelyeket erinteni kell:
- `backend/src/trading/orchestrator.rs`
- `backend/src/trading/bot_executor/executor.rs`
- `backend/src/trading/bot_executor/strategies.rs`
- `backend/src/trading/polymarket.rs`
- `backend/src/api/bots.rs`
- `backend/src/api/orders.rs`
- `backend/src/api/settings.rs`
- `backend/src/api/monitoring.rs`
- `backend/src/db/mod.rs`

## Demo mode kovetelmenyek

### Funkcionalis kovetelmenyek

- Demo bot inditasa credential nelkul is mukodjon.
- Demo balance legyen explicit bot config/run parameter, ne legyen egyszer `$10`, maskor `$100`.
- Demo order lifecycle legyen valosagkozeli:
  - intent created
  - simulated order submitted
  - simulated fill price rogzitve
  - position opened
  - market settled
  - PnL calculated
  - run metrics updated
- Egy demo run ne irja felul automatikusan a korabbi eredmenyeket, hacsak a user nem nyom resetet.
- Legyen kulon "Strategy Test Run", amely nem feltetlenul indit folyamatos botot, hanem egy adott strategiat tesztel egy replay/current market ablakon.

### Demo szimulacios szabalyok

Elso korben egyszeru, de kovetkezetes szabalyok:
- Fill price = aktualis YES/NO midpoint plus opcionális slippage.
- Minimum es maximum order size ugyanazzal a risk layerrel ellenorizve, mint live modban.
- Ha odds hianyzik vagy market snapshot invalid, trade legyen `rejected`, ne csendes `hold`.
- Settlement:
  - YES nyer, ha final BTC price >= price_to_beat.
  - NO nyer, ha final BTC price < price_to_beat.
  - PnL szamitas a megvasarolt share mennyisegre epuljon, ne csak a bet size levonasara.

### Strategy Lab

Uj frontend/backend workflow:
1. User kivalaszt egy strategiat.
2. Beallitja a parametereket.
3. Valaszt:
   - current live market simulation
   - last N completed 5m windows replay
   - saved historical sample
4. Elinditja a tesztet.
5. Latja:
   - trade intenteket
   - miert volt hold/reject/trade
   - fill arakat
   - settlementeket
   - win rate, ROI, max drawdown, avg pnl, trade count

## Live mode kovetelmenyek

### Live preflight

Live bot csak akkor indulhat, ha minden check zold:
- credential decrypt es API key valid.
- private key valid.
- wallet/funder address ismert.
- USDC/POL vagy aktualis Polymarket collateral balance elegendo.
- allowance rendben vagy egyertelmu hiba jelenik meg.
- gas/funding check lefut.
- market token id-k validak.
- bot risk config nem enged veszelyes meretet.

### Live execution

- A live adapter ne adjon vissza szimulalt order id-t.
- A Polymarket CLOB kliens kapja meg az API key, secret, passphrase, private key, funder es signature_type adatokat.
- Order request elott legyen price bounds, size bounds, balance check es time remaining check.
- Order utan legyen status sync:
  - submitted
  - open
  - partially_filled
  - filled
  - canceled
  - rejected
  - expired
- Hibak legyenek strukturaltak:
  - `credentials_missing`
  - `insufficient_balance`
  - `allowance_missing`
  - `market_not_tradeable`
  - `price_out_of_bounds`
  - `clob_rejected`
  - `network_timeout`

## Adatmodell terv

### Uj vagy atalakitott tablak

1. `bot_runs`
   - Egy bot inditas/futas egysege.
   - Mezo javaslatok: `id`, `bot_id`, `user_id`, `mode`, `status`, `started_at`, `ended_at`, `initial_balance`, `final_balance`, `realized_pnl`, `unrealized_pnl`.

2. `trade_intents`
   - Strategia altal javasolt kereskedes, akkor is, ha vegul reject vagy hold lett.
   - Mezo javaslatok: `run_id`, `market_slug`, `strategy_type`, `side`, `confidence`, `reason`, `snapshot_json`, `risk_result`.

3. `executions`
   - Paper vagy live order konkret vegrehajtasa.
   - Mezo javaslatok: `intent_id`, `mode`, `adapter`, `status`, `token_id`, `side`, `requested_size`, `filled_size`, `requested_price`, `avg_fill_price`, `external_order_id`, `error_code`.

4. `positions`
   - Maradhat, de legyen mode/run kapcsolata.
   - Demo es live poziciok ne keveredjenek.

5. `market_snapshots`
   - Opcionálisan cache-elt/replayelheto market adat.
   - Strategy Labhoz erosen ajanlott.

6. `bot_events`
   - Event journal per bot/run/test.
   - Innen epuljon az activity feed es a debug timeline.

### Migracios megkozelites

- Eloszor additiv migration: uj tablak, regi tablak megtartasa.
- Ezutan adapter reteg irjon az uj tablakba.
- Frontend fokozatosan az uj API-kra valt.
- Regi mezok olvasasa csak kompatibilitasi okbol maradjon egy atmeneti fazisig.

## API terv

### Bot es run API

- `GET /api/trading/modes/readiness`
  - Visszaadja: demo_ready, live_ready, missing live prerequisites.

- `POST /api/bots/:id/start`
  - Explicit body: `{ "mode": "demo" | "live", "initial_balance": number }`.
  - Ne a bot config rejtett allapota dontse el egyedul a modot.

- `POST /api/bots/:id/stop`
  - Leallitja a run managert, es opcionálisan live modban cancel open orders.

- `GET /api/bots/:id/runs`
  - Run history.

- `GET /api/runs/:id/events`
  - Debug timeline.

- `GET /api/runs/:id/performance`
  - PnL, win rate, drawdown, trade distribution.

### Strategy Lab API

- `GET /api/strategies`
  - Strategiak listaja, param schema, default paramok, rovid leiras.

- `POST /api/strategy-tests`
  - Letrehoz egy demo/replay tesztet.

- `GET /api/strategy-tests/:id`
  - Teszt status es osszesitett eredmeny.

- `GET /api/strategy-tests/:id/events`
  - Lepesenkenti dontesek es okok.

### SSE contract

SSE event tipusok legyenek verziozottak:
- `market.snapshot.v1`
- `bot.run_started.v1`
- `bot.strategy_evaluated.v1`
- `bot.order_submitted.v1`
- `bot.order_filled.v1`
- `bot.order_rejected.v1`
- `bot.position_updated.v1`
- `bot.market_settled.v1`
- `bot.run_stopped.v1`
- `system.health.v1`

Minden event tartalmazza:
- `event_id`
- `server_timestamp`
- `seq`
- `user_id` vagy szerver oldali user-scope
- `bot_id`, ha relevans
- `run_id`, ha relevans
- `mode`, ha relevans

## Frontend celallapot

### Informacios architektura

Az app ne egyetlen tulzsufolt dashboard legyen. Javasolt oldalak:

1. `/`
   - Operator Dashboard.
   - Csak a most fontos allapotok:
     - active market
     - demo/live readiness
     - running bots
     - open positions
     - latest critical events
     - emergency stop

2. `/strategy-lab`
   - Demo strategia teszteles kozpont.
   - Strategy selector, parameter editor, run controls, results, event timeline.

3. `/bots`
   - Bot fleet kezeles.
   - Tabla vagy suru lista, nem dekorativ kartyarengeteg.
   - Start demo/live kulon action, status, latest run, PnL.

4. `/runs`
   - Run history es osszehasonlitas.
   - Demo es live eredmenyek szurovel elvalasztva.

5. `/markets`
   - Active BTC 5m piac, price-to-beat, odds, countdown, liquidity.
   - Market snapshot debug nezet.

6. `/orders`
   - Live order lifecycle es demo executions egy helyen, mode filterrel.

7. `/settings`
   - Credentials, live readiness, risk defaults, data/debug settings.

### Dashboard layout

Javasolt desktop elrendezes:

- Felso status bar:
  - mode selector: Demo / Live
  - live readiness badge
  - BTC price
  - price to beat
  - countdown
  - SSE/API health

- Fo tartalom, 3 oszlop:
  - Bal: running bots compact table
  - Kozep: active market + chart + current positions
  - Jobb: event timeline + alerts

- Also sav:
  - performance summary
  - last completed markets
  - recent fills/rejections

### Design irany

- Kevesebb glassmorphism es kevesebb dekorativ glow.
- Sotet tema maradhat, de legyen semlegesebb, trading-tool jellegu.
- A kartyak helyett suru, rendezett panelek es tablak.
- Mode szinek kovetkezetesen:
  - Demo: indigo/kek jeloles, mindenhol "Demo" cimke.
  - Live: zold/piros riziko jeloles, de live actionoknal eros figyelmeztetes.
- A fo actionok ne tunjenek egyformanak:
  - Start Demo
  - Start Live
  - Stop
  - Reset Demo
  - Emergency Stop
- A frontend ne hasznaljon emoji ikonokat status jelolesre; Lucide ikonok es text badge-ek legyenek.
- A szamok monospace fonttal, fix szelessegu blokkokban jelenjenek meg, hogy ne ugraljon a layout.
- Mobilon ne legyen minden funkcio egyszerre jelen: status, active bots, emergency stop es latest events legyen prioritas.

### Fontos komponensek

Uj vagy atalakitott komponensek:
- `frontend/src/components/mode/mode-switcher.tsx`
- `frontend/src/components/mode/live-readiness-panel.tsx`
- `frontend/src/components/market/active-market-strip.tsx`
- `frontend/src/components/bots/bot-fleet-table.tsx`
- `frontend/src/components/bots/bot-run-controls.tsx`
- `frontend/src/components/strategy-lab/strategy-selector.tsx`
- `frontend/src/components/strategy-lab/strategy-params-editor.tsx`
- `frontend/src/components/strategy-lab/test-run-panel.tsx`
- `frontend/src/components/strategy-lab/test-results.tsx`
- `frontend/src/components/events/event-timeline.tsx`
- `frontend/src/components/orders/execution-table.tsx`

Atalakitasra jelolt komponensek:
- `frontend/src/components/dashboard/command-center.tsx`
- `frontend/src/components/dashboard/bot-selector.tsx`
- `frontend/src/components/dashboard/trade-feed.tsx`
- `frontend/src/components/dashboard/strategy-performance.tsx`
- `frontend/src/components/dashboard/system-health.tsx`
- `frontend/src/store/index.ts`
- `frontend/src/hooks/use-sse.ts`
- `frontend/src/hooks/use-api.ts`

## Allapotkezeles frontend oldalon

### Zustand

Zustand maradhat UI es real-time allapotra:
- selected mode
- current market snapshot
- SSE connection status
- active bot run ids
- latest event timeline
- local UI preferences

### React Query

React Query kezelje a szerver igazsag-forras adatokat:
- bots
- runs
- strategy tests
- orders/executions
- positions
- readiness
- settings

### SSE

- Egyetlen provider-szintu SSE kapcsolat legyen.
- Az SSE ne tartson fenn uzleti logikat, csak normalizalt eventeket dispatch-eljen.
- Reconnect utan a frontend kerjen REST snapshotot:
  - active market
  - running bots
  - open positions
  - latest events
- Eventek deduplikalasa `event_id` vagy `seq` alapjan.

## Megvalositasi utemterv

### Fazis 0: Alap stabilizalas

Cel: ne epitkezzunk instabil alapra.

- Dönteni kell, hogy a `BotOrchestrator` marad-e kanonikus engine. Javaslat: igen, de `RunManager` neven egyszerusitve; a regi `BotExecutor` legyen torolve vagy archive-olva, ha nincs aktiv hasznalatban.
- Javítani kell a portfolio query-k user-scope es bind param problemakat.
- Egységesiteni kell a mode nevezektant: backend es frontend mindenhol `demo` es `live`, DB-ben migrationnal kompatibilis alias a regi `paper` ertekre.
- Zolditeni kell:
  - `cd backend && cargo test`
  - `cd frontend && bun run lint`
  - `cd frontend && bun run build`

### Fazis 1: Market data es strategy contract

Cel: a strategia mindig ugyanazt az inputot kapja.

- Kiemelni az aktiv market discoveryt az SSE-bol egy `MarketDataService`-be.
- Letrehozni a normalizalt `MarketSnapshot` contractot.
- A strategy evaluation kapjon teljes contextet:
  - btc_price
  - price_to_beat
  - yes/no odds
  - time_remaining
  - market_slug
  - token ids
  - recent BTC history
- A `Hold` is tartalmazzon kodolt okot, ne csak szabad szoveget.

### Fazis 2: Demo execution engine

Cel: a demo mode legyen a strategia teszteles megbizhato alapja.

- Letrehozni a `PaperExecutionAdapter`-t.
- Minden demo trade intentbol execution record keszuljon.
- Settlement kulon service-ben fusson market transitionkor.
- Demo run history ne vesszen el bot restartkor.
- Reset demo balance kulon action legyen, explicit megerositessel.
- Bulk run es single run ugyanazt az initial balance szabalyzatot hasznalja.

### Fazis 3: Strategy Lab

Cel: ne csak "bot inditassal" lehessen strategiat tesztelni.

- Backend API a strategy test run letrehozasara.
- Current-market test: az aktualis 5 perces piacra futtat.
- Replay test: mentett market snapshotokon fut.
- Frontend `/strategy-lab` oldal:
  - strategy valaszto
  - param editor
  - initial balance/slippage beallitas
  - run gomb
  - eredmeny panel
  - dontesi timeline
- A test eredmeny legyen osszehasonlithato mas strategy testekkel.

### Fazis 4: Live execution biztonsagos bekotese

Cel: csak akkor kereskedjen elesben, ha minden eloellenorzes rendben van.

- `LivePolymarketExecutionAdapter` bekotese a Polymarket CLOB kliensre.
- Credential service bovites, hogy teljes API credentiallel tudjon klienst adni, ne csak private key alapjan.
- Live readiness API.
- Allowance, balance, gas/funding, market tradeability check.
- Order status sync es structured errors.
- Emergency stop: running bots stop + open order cancel kiserlet.

### Fazis 5: Frontend ujrarendezes

Cel: operatori, tiszta, mode-aware UI.

- Dashboard egyszerusitese, fontos allapotok felulre.
- Bot fleet tabla `Start Demo` es `Start Live` kulon actionokkal.
- Mode switch ne csak frontend preference legyen, hanem API actionokban explicit parameter.
- Strategy Lab kulon oldal.
- Event timeline legyen az elso szamu debug es magyarazati felulet.
- System Health panel ne legyen eldugott: live mode elott kotelezo readiness panel.
- A jelenlegi collapsible panel struktura csokkentese; csak masodlagos blokkok legyenek csukhatoak.

### Fazis 6: Observability es minosegkapuk

Cel: hibakat gyorsan lehessen ertelmezni.

- Structured tracing backend oldalon:
  - `run_id`
  - `bot_id`
  - `mode`
  - `market_slug`
  - `intent_id`
  - `execution_id`
  - `external_order_id`
- Frontend hiba panel:
  - utolso API hiba
  - SSE reconnect status
  - live readiness missing items
  - rejected trade reasonok
- CI vagy lokalis release checklist:
  - `cd backend && cargo test`
  - `cargo build --release --manifest-path backend/Cargo.toml`
  - `cd frontend && bun run lint`
  - `cd frontend && bun run build`

## Tesztelesi terv

### Backend unit tesztek

- StrategyEngine:
  - valid contextbol trade intent
  - tul korai/tul kesoi piacnal hold
  - odds bounds reject
  - hianyzo BTC/market adat reject/hold kod

- PaperExecutionAdapter:
  - buy YES szimulalt fill
  - buy NO szimulalt fill
  - insufficient demo balance
  - slippage alkalmazas
  - settlement win/loss PnL

- RunManager:
  - demo bot inditas credential nelkul
  - live bot inditas credential nelkul elutasitva
  - stop run lezart statuszt eredmenyez
  - user-scoped bot/run lekerdezes

### Backend integration tesztek

- `POST /api/bots/:id/start` demo modban letrehoz run-t es eventet.
- `POST /api/bots/:id/start` live modban credential hianyra strukturalt hibat ad.
- Strategy test API teljes flow: create -> events -> result.
- SSE event contract tartalmazza a kotelezo mezoket.

### Frontend tesztek es ellenorzes

- TypeScript build: `cd frontend && bun run build`.
- Biome lint: `cd frontend && bun run lint`.
- Kezi browser check:
  - nincs login hydration mismatch
  - SSE egyszer csatlakozik
  - demo/live mode minden actionnel egyertelmu
  - Strategy Lab run utan eredmeny latszik
  - live readiness hianyos credentialnel ertheto hibat mutat
  - mobilon nincs szoveg- vagy panel-overlap

## Kockazatok es dontesi pontok

- Polymarket API V2 reszletek valtozhatnak, ezert live execution elott hivatalos dokumentacio szerinti endpoint/signature ellenorzes kell.
- A jelenlegi DB migration stilus additiv es inline; nagyobb schema valtozasoknal erdemes kulon migration rendszert bevezetni.
- Replay/backtest csak akkor lesz hasznos, ha megbizhato market snapshot adatot gyujtunk.
- Ha live order status sync nem teljes, akkor frontend false positive "order executed" allapotot mutathat. Ezt tiltani kell: csak confirmed API valasz utan legyen filled/executed.
- A tul sok strategia egyszerre zavaro. A Strategy Labban elso korben 3-5 valoban hasznalhato strategiat erdemes kiemelni, a tobbit "advanced" szuro moge tenni.

## Javasolt prioritas

1. Stabilizalas es kanonikus engine kijelolese.
2. Demo mode teljes ujraalapozasa normalizalt market snapshot + paper execution + settlement menten.
3. Strategy Lab elkeszitese, mert ez validalja, hogy a demo mode tenyleg hasznalhato.
4. Frontend dashboard ujrarendezese mode-aware operatori feluletre.
5. Live execution bekotese csak readiness, risk es order status sync utan.

## Kesz allapot definicio

A projekt akkor tekintheto rendben levo demo/live rendszernek, ha:

- Demo bot credential nelkul indithato, kovetkezetes initial balance-szal.
- Demo strategy test futtathato kulon Strategy Lab oldalon.
- Minden trade donteshez latszik, hogy miert lett hold, reject vagy trade.
- Demo settlement es PnL reprodukalhato es run historyban megmarad.
- Live bot nem indul el hianyos credential/balance/allowance/gas/readiness eseten.
- Live order csak valos Polymarket order response utan jelenik meg orderkent.
- Frontend egyertelmuen elkuloniti a Demo es Live allapotot.
- Dashboardon 5 masodpercen belul ertheto:
  - fut-e bot
  - milyen modban fut
  - mi az aktiv market
  - van-e nyitott pozicio/order
  - volt-e friss hiba
- `cargo test`, backend release build, frontend lint es frontend build zold.
