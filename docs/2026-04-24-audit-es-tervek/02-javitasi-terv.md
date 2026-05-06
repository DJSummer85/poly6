# Javítási Terv

Dátum: 2026-04-24

## Cél

Stabil, kiszámítható, többfelhasználósan is korrektül működő alapot létrehozni, amire később biztonságosan lehet új funkciókat építeni.

## Prioritási elv

- `P0`: hibák, amelyek rossz működést, adatbiztonsági gondot vagy félrevezető viselkedést okoznak
- `P1`: stabilitási és karbantarthatósági adósság
- `P2`: minőségi és fejlesztői élmény javítások

## P0 feladatok

### P0.1 Credential unlock flow egységesítése

Érintett területek:
- `backend/src/api/settings.rs`
- `backend/src/api/orders.rs`
- `backend/src/api/auth.rs`
- szükség esetén új `backend/src/services/credentials.rs`

Teendők:
- Meg kell szüntetni a hardcode-olt `"techno"` jelszóhasználatot.
- Közös credential service kell a store/decrypt/validate/trade lépésekhez.
- Ki kell választani az unlock modellt:
  - session-alapú decrypt kulcs
  - explicit "unlock trading credentials" flow
  - rövid élettartamú memóriabeli credential cache
- A kézi order, quick trade és bot live trade ugyanazt az útvonalat használja.

Elvárt eredmény:
- Nincs eltérő decrypt logika endpointonként.
- A mentett credential biztosan feloldható ugyanazzal a folyamattal, amivel el lett mentve.

### P0.2 Backend tesztek zöldítése

Érintett területek:
- `backend/src/trading/polymarket.rs`
- `backend/src/crypto/mod.rs`
- szükség esetén új teszt helper modul

Teendők:
- A hibás placeholder alapú tesztet javítani vagy lecserélni.
- A kritikus trading/client/auth utilok köré minimál megbízható unit tesztet írni.
- A cargo test legyen alap követelmény minden változtatás előtt és után.

Elvárt eredmény:
- `cargo test` zöld.
- A jelenlegi piros teszt nem zajosítja tovább a fejlesztést.

### P0.3 Multi-user scope korrekció

Érintett területek:
- `backend/src/trading/orchestrator.rs`
- `backend/src/api/bots.rs`
- `backend/src/api/monitoring.rs`
- esetleg SSE státusz összesítések

Teendők:
- `get_running_bots(user_id)` ténylegesen szűrjön felhasználóra.
- Minden bot/session/portfolio nézet legyen user-scoped.
- Ellenőrizni kell, hogy nincs-e más globális állapot, ami usertől függetlenül szivárog.

Elvárt eredmény:
- Egy user csak a saját futó botjait és állapotait lássa.

### P0.4 SSE market transition stabilizálása

Érintett területek:
- `frontend/src/hooks/use-sse.ts`
- `frontend/src/store/index.ts`
- releváns dashboard komponensek

Teendők:
- `startPrice` csak valid új értéknél frissüljön.
- Különítsük el a "last confirmed" és "incoming" state-eket.
- A market history mentése ne 0-s target árakkal történjen.
- Ellenőrzött UI fallback kell az új market első 1-2 eventjére.

Elvárt eredmény:
- nincs price flicker
- nincs átmeneti 0-s target price
- a market history konzisztens marad

## P1 feladatok

### P1.1 Frontend lint teljes rendezése

Teendők:
- import/order és formatter hibák javítása
- `useEffect` dependency hibák rendezése
- `label` és input kapcsolatok rendbetétele
- nem használt változók eltávolítása

Elvárt eredmény:
- `cd frontend && bun run lint` zöld

### P1.2 `unwrap()` és panic audit

Teendők:
- SSE, settings, auth, trading kliens és API route-ok auditja
- kerüljenek be strukturált hibaválaszok és guard ágak

Elvárt eredmény:
- kevesebb runtime pánik
- jobban diagnosztizálható hibák

### P1.3 Trading flow dokumentálása és konszolidálása

Teendők:
- rögzíteni kell a kanonikus végrehajtási útvonalat:
  - manual order
  - quick trade
  - bot live trade
- a duplikált credential és order-building logikát közös szolgáltatásba kell mozgatni

Elvárt eredmény:
- kevesebb duplikáció
- kisebb regressziós felület

### P1.4 Halott kód és félkész modulok tisztítása

Teendők:
- az unused exportok és régi trading modulok áttekintése
- el kell dönteni, melyik executor/strategy/orchestrator útvonal az aktív
- ami nem aktív, azt törölni vagy archiválni kell

Elvárt eredmény:
- karcsúbb, áttekinthetőbb backend

## P2 feladatok

### P2.1 Minőségkapuk bevezetése

Javaslat:
- backend: `cargo test`
- frontend: `bun run lint`
- frontend: `bun run build`

Ha van CI:
- ezt kötelező PR kapunak érdemes tenni

### P2.2 Megfigyelhetőség javítása

Teendők:
- strukturált logok a trade lifecycle mentén
- credential hibák, market transitionök, order failure okok külön log mezőkkel

### P2.3 Fejlesztői workflow javítása

Teendők:
- rövid `CONTRIBUTING` vagy fejlesztői checklista
- ajánlott napi parancsok
- "known issues" lista

## Javasolt ütemezés

### Sprint 1

- P0.1 Credential unlock flow
- P0.2 Backend tesztek zöldítése
- P0.3 Multi-user scope javítás

### Sprint 2

- P0.4 SSE transition stabilizálása
- P1.1 Frontend lint rendezése
- P1.2 Panic audit első köre

### Sprint 3

- P1.3 Trading flow konszolidálása
- P1.4 Cleanup
- P2 minőségkapuk

## Definíció kész állapotra

Ezt tekintsük az alap stabilizáció végét jelentő állapotnak:

- backend tesztek zöldek
- frontend lint zöld
- frontend build zöld
- nincs hardcode-olt decrypt jelszó
- manual trade, quick trade és live bot ugyanazzal a hiteles credential flow-val működik
- a bot listing és futó bot állapot user-scoped
- a market transition UI nem villog és nem nulláz indokolatlanul
