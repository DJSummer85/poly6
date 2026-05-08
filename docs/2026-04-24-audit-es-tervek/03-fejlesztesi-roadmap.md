# Fejlesztési Roadmap

Dátum: 2026-04-24

## Cél

Olyan fejlesztési sorrend kialakítása, amely először a jelenlegi rendszer köré épít megbízható alapot, majd fokozatosan növeli a kereskedési képességet, az átláthatóságot és az automatizálási szintet.

## Alapelv

Új funkció csak akkor kerüljön sorra, ha az előtte lévő stabilitási szint megvan. Egy trading botnál a látványos feature kevesebbet ér, mint a megbízható végrehajtás, a jó állapotkövetés és a kiszámítható hibakezelés.

## Fázis 1: Stabil alapok

### 1. Trading Core hardening

Funkcióötletek:
- egységes order execution service
- credential unlock lifecycle
- explicit paper vs live execution state
- order failure retry policy
- fail-safe bot stop mechanizmus

Miért fontos:
- ez lesz minden további funkció alapja

### 2. Portfolio és PnL pontosság

Funkcióötletek:
- realized és unrealized PnL külön kezelése
- session szintű equity curve
- bot drawdown és win-rate tisztázott definícióval
- nyitott pozíciók valós idejű értékelése

Miért fontos:
- jelenleg a bot teljesítménye és a felhasználói bizalom csak akkor erős, ha a számok hihetőek és auditálhatók

### 3. Dashboard megbízhatóság

Funkcióötletek:
- stabil SSE reconnect state
- market transition jelölése
- bot health státuszok
- külön "data stale" jelzés, ha a stream régi

Miért fontos:
- a UI ne csak szép legyen, hanem operatív felületként is használható

## Fázis 2: Operátori eszközök

### 4. Bot kontrollközpont

Funkcióötletek:
- egységes command center
- start, stop, emergency stop, pause, resume
- botonkénti kockázati limitek
- session előzmények és döntésnapló

Miért fontos:
- a felhasználó ne több oldalon vadássza össze az állapotot

### 5. Order és execution monitor

Funkcióötletek:
- pending, filled, partially filled, failed státuszok
- slippage és fill latency nézet
- order timeline
- order retry és failure reason panel

Miért fontos:
- egy trading rendszerben a végrehajtási láthatóság majdnem olyan fontos, mint maga a stratégia

### 6. Audit trail

Funkcióötletek:
- bot döntés -> order -> fill -> PnL lánc összekapcsolása
- exportálható kereskedési napló
- admin/debug események külön szinten

## Fázis 3: Stratégiai fejlesztések

### 7. Stratégia összehasonlító nézet

Funkcióötletek:
- stratégia baseline összehasonlítás
- session eredmények stratégia szerint
- paraméter variációk összevetése

### 8. Paraméterezhető risk engine

Funkcióötletek:
- max exposure
- max concurrent positions
- napi loss limit
- cooling period vesztes széria után
- confidence threshold tuning

### 9. Backtest és replay

Funkcióötletek:
- historical replay a BTC up/down marketekre
- strategy replay adott időablakon
- dry-run order simulation

Megjegyzés:
- ezt csak a core stabilizálása után érdemes elkezdeni

## Fázis 4: Product szintű bővítések

### 10. Multi-market támogatás

Lehetséges irányok:
- más timeframe-ek
- több Polymarket eseménytípus
- market selector és szűrők

### 11. Felhasználói élmény fejlesztése

Funkcióötletek:
- onboarding flow
- settings validation wizard
- live trading biztonsági megerősítések
- "paper mode ajánlott" guard az első indulásnál

### 12. Jelentések és értesítések

Funkcióötletek:
- napi összefoglaló
- vesztes limit elérés értesítés
- order failure alert
- bot leállás figyelmeztetés

## Javasolt feature backlog sorrend

1. Egységes order execution service
2. Pontos portfolio és PnL számítás
3. Stabil command center
4. Order monitor és audit trail
5. Risk engine
6. Backtest és replay
7. Multi-market támogatás
8. Értesítések és riportok

## Miket nem érdemes még most építeni

- túl komplex stratégia marketplace
- automatikus self-tuning rendszer
- sok új market támogatása egyszerre
- látványos UI refactor a core stabilitás előtt

Ezek most könnyen elfednék a fontosabb stabilitási hiányosságokat.

## Ajánlott következő konkrét epicek

### Epic A: Trading Foundation

Tartalom:
- credential flow
- order execution service
- test green baseline

### Epic B: Reliable Operator UI

Tartalom:
- stable SSE
- command center
- order/portfolio láthatóság

### Epic C: Strategy Confidence Layer

Tartalom:
- risk engine
- performance analytics
- replay/backtest előkészítés
