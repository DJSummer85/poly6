# Polymarket Kereskedő Bot - Átfogó Kód Review és Fejlesztési Terv

Ezt a dokumentumot a kód bázisának átvizsgálása alapján készítettem, hogy összefoglaljam a jelenlegi hibákat, a javítandó funkciókat, és egy strukturált fejlesztési tervet adjak.

## 1. Jelenlegi Hibák és Kritikus Hiányosságok

### 1.1. A botok nem kötnek élesben (Live Execution Bug)
A legkritikusabb hiba a `backend/src/trading/orchestrator.rs` fájlban található. A `execute_cycle` függvény lefut, kiszámolja a stratégiát, a Kelly-kritérium alapján a tétet, sőt le is naplózza az adatbázisba a döntést, de **nem küldi be az order-t a Polymarket felé**. 
A kód 394. sorában jelenleg ez áll: `// TODO: Execute actual order if in live mode`.
**Javítás:** Be kell kötni a `PolymarketClient::create_order` és `post_order` függvényeket a döntési ciklus végére.

### 1.2. MATIC (Gázdíj) fedezet ellenőrzésének hiánya
A `polymarket.rs` fájl tartalmazza a USDC egyenleg ellenőrzését (`get_balance`), de nem vizsgálja a Polygon hálózaton szükséges MATIC egyenleget. Ha a tárcában nincs elég MATIC a gázdíjakra, a Polymarket contract hívások el fognak bukni a hálózaton (Failing transactions).
**Javítás:** RPC hívással (ethers.js / ethers-rs segítségével) lekérdezni a MATIC egyenleget a kereskedési ciklus előtt, és leállítani/figyelmeztetni a botot, ha ez egy kritikus szint alá esik.

### 1.3. UI Villogás ("Price to beat flickering")
A frontend oldalon a `frontend/src/hooks/use-sse.ts` felelős a valós idejű adatokért. Amikor új 5-perces piac indul, a kód nullázza a `prevStartPriceRef.current` értéket, ami miatt a UI-on a célarfolyam egy pillanatra eltűnik vagy ugrál, mire megérkezik az új adat.
**Javítás:** A nullázás helyett egy folytonos átmenetet kell biztosítani, illetve a Zustand store-ban az előző piac végső árát kell "price to beat"-ként megtartani az új adat megérkezéséig.

### 1.4. Teljesítménymetrikák és Számítási Hibák
A botok teljesítményének számításakor a rendszer (a `get_portfolio` DB hívásban) gyakran csak a realizált (készpénzes) egyenleget veszi figyelembe, és nem értékeli helyesen a **nyitott pozíciók valós idejű értékét**. Emiatt a bot performanciája látszólag visszaesik, amíg egy piac le nem zárul.
**Javítás:** A PnL és Balance számításokba be kell vonni a `current_value`-t a megvásárolt, de még le nem zárt tokeneknél.

### 1.5. SSE Redundáns Adatlekérés
A Zustand store és az SSE kapcsolat nincs singleton mintával védve. Ha a `useSSE` hook több komponensben is meghívásra kerül (vagy az oldal újratöltődik), több párhuzamos kapcsolat is nyílhat a backend felé.
**Javítás:** Az SSE kapcsolatot a legfelső (Provider) szinten kell kezelni, vagy globális ref-ként eltárolni, elkerülve a redundáns csatlakozásokat.

---

## 2. Fejlesztési Terv és Architektúra Javítások

### Fázis 1: Kritikus Trading Logika Javítása (Backend)
1. **Éles kereskedés beépítése:** Az `orchestrator.rs`-ben a `TradeDecision` után meghívni a `polymarket_client`-et. Meg kell különböztetni a "Paper Trading" (szimulált) és "Live Trading" (éles) botokat.
2. **MATIC Balance Checker implementálása:** Egy aszinkron checker létrehozása, ami a bot indításakor és bizonyos időközönként validálja a gázdíjat.
3. **Hibakezelés (Error Handling):** Sikertelen tranzakció esetén (pl. API hiba, timeout) a bot ne fagyjon ki, hanem alkalmazzon retry mechanizmust, vagy álljon le biztonságosan.

### Fázis 2: UI/UX Javítások (Frontend)
1. **Flickering Fix:** Az `use-sse.ts` átírása, robusztusabb piacváltás detektálással.
2. **Command Center Unified Design:** A `bot-grid`, `compact-data-bar` és a különálló menük egybeolvasztása egy modern, letisztult Command Center-ré.
3. **Nyitott Pozíciók Megjelenítése:** Valós idejű nyereség/veszteség (Unrealized PnL) kijelzése a folyamatban lévő 5-perces piacoknál.

### Fázis 3: Stratégia és Teljesítmény Optimalizálás
1. **Bot Metrikák Pontosítása:** A drawdown és win-rate kalkulációk átírása úgy, hogy azok pontosan tükrözzék az edge case-eket is.
2. **Auto-tuning paraméterek:** Későbbi fejlesztésként egy backtest/elemző funkció, ami a vesztő stratégiák paramétereit (pl. btc_window_open, stop-loss) proaktívan javasolja megváltoztatni.

## Összegzés a teendőkhöz:
A projekt alapjai nagyon jók, a Next.js és Rust párosítás kiváló teljesítményt nyújt. A legfontosabb, hogy a bot motor végrehajtsa a tényleges tranzakciókat, és a frontend stabilan mutassa a valós adatokat anélkül, hogy ugrálna a UI.
