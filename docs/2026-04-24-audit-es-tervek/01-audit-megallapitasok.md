# Audit Megállapítások

Dátum: 2026-04-24

## Ellenőrzési eredmények

### Backend

Parancs: `cd backend && cargo test`

Eredmény:
- 4 teszt átment
- 1 teszt elbukott

Konkrét hiba:
- [backend/src/trading/polymarket.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/trading/polymarket.rs:661) `test_client_creation`
- A teszt egy placeholder kulcsot próbál valós privát kulcsként felhasználni, ezért `unwrap()` pánikkal elhasal.

### Frontend

Parancs: `cd frontend && bun run lint`

Eredmény:
- 41 hiba
- 6 warning

Jellemző hibacsoportok:
- import rendezési problémák
- formázási eltérések
- `useEffect` dependency hibák
- accessibility hibák (`label` nincs inputhoz kötve)
- felesleges változók

Parancs: `cd frontend && bun run build`

Eredmény:
- sikeres production build

Következtetés:
- A projekt fordul, de nincs jó minőségi kapu köré építve.

## Kritikus problémák

### 1. Hardcode-olt jelszó a credential decrypthez

Érintett helyek:
- [backend/src/api/orders.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/api/orders.rs:132)
- [backend/src/api/orders.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/api/orders.rs:581)

Megfigyelés:
- A kód a `"techno"` jelszót használja a titkosított credential blob visszafejtéséhez.
- A Settings API ezzel szemben felhasználói jelszóra támaszkodik.

Kockázat:
- A kézi order placement és a quick trade funkció élesben hibásan működhet.
- A rendszer félrevezető lehet: a credential látszólag el van mentve, de a trade endpoint később nem tudja feloldani.
- Ez egyszerre működési és biztonsági probléma.

Javasolt irány:
- A decrypthez szükséges kulcsképzést központi auth/credential service-be kell szervezni.
- A trade endpointok ne tartalmazzanak saját jelszófeltételezést.
- A sessionből vagy explicit unlock flow-ból kell származnia az aktív decrypt kulcsnak.

### 2. A backend tesztcsomag jelenleg eleve piros

Érintett hely:
- [backend/src/trading/polymarket.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/trading/polymarket.rs:661)

Megfigyelés:
- A `test_client_creation` nem stabil tesztadatot használ, hanem placeholder értéket és `unwrap()`-ot.

Kockázat:
- Minden tesztfuttatás hamis negatív eredményt ad.
- A hibakeresés zajosabb lesz, mert egy ismerten rossz teszt folyamatosan pirosan tartja a csomagot.

Javasolt irány:
- Legyen determinisztikus unit teszt fix, valid formátumú mock/private key mintával vagy mockolt wallet generálással.
- `unwrap()` helyett explicit assertion és leíró hibaüzenet kell.

### 3. Többfelhasználós botkezelés hiányos

Érintett hely:
- [backend/src/trading/orchestrator.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/trading/orchestrator.rs:199)

Megfigyelés:
- A `get_running_bots(&self, user_id)` paramétert a metódus nem használja, és minden futó bot ID-t visszaad.

Kockázat:
- User scope szivárgás
- Hibás adminisztrációs és dashboard állapot
- Későbbi multi-user hiba vagy jogosultsági probléma

Javasolt irány:
- `RunningBot.user_id` már létezik, erre kell ténylegesen szűrni.
- Minden orchestrator és API olvasásnál következetesen user-scoped lekérdezést kell használni.

### 4. A frontend market transition javítás félkész

Érintett hely:
- [frontend/src/hooks/use-sse.ts](/Users/bandi/Documents/Code/2026/polymarket-trader/frontend/src/hooks/use-sse.ts:54)

Megfigyelés:
- A hook eltárolja az előző `startPrice` értéket, de utána a store-ba mégis a frissen kapott `newStartPrice` kerül.
- Ha az új market elején ez még 0 vagy hiányzik, a store átmenetileg 0-ra áll.

Kockázat:
- villogó vagy inkonzisztens UI
- hibás `price to beat` megjelenítés
- torz market history számítás

Javasolt irány:
- A megjelenített `startPrice` és `beatPrice` csak valid új érték esetén frissüljön.
- Be kell vezetni explicit transition state-et: `pending_market_transition`, `last_confirmed_start_price`.

### 5. Sok kockázatos `unwrap()` és részben félkész logika maradt bent

Érintett példák:
- [backend/src/api/sse.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/api/sse.rs:69)
- [backend/src/api/settings.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/api/settings.rs:450)
- [backend/src/middleware/auth.rs](/Users/bandi/Documents/Code/2026/polymarket-trader/backend/src/middleware/auth.rs:101)

Megfigyelés:
- Több helyen van fölösleges `unwrap()` ott, ahol az adat már egyszer opcióként lett kezelve.
- A kód működik sok happy-path esetre, de sebezhetőbb hibaágak és edge case-ek mellett.

Kockázat:
- váratlan pánikok
- nehezen reprodukálható production hibák
- rossz hibatűrés külső API ingadozás esetén

Javasolt irány:
- `unwrap()` audit és fokozatos kiszorítás
- strukturált hibatípusok
- normalizált fallback útvonalak az SSE, auth és settings körül

## Magas prioritású problémák

### 6. Frontend lint és a11y adósság

Érintett helyek:
- `frontend/app/login/page.tsx`
- `frontend/app/register/page.tsx`
- `frontend/app/bots/page.tsx`
- `frontend/app/markets/page.tsx`
- `frontend/app/providers.tsx`

Megfigyelés:
- Több űrlapcímke nincs inputhoz kapcsolva.
- Több effect dependency hiányzik.
- A formázási és import hibák tömegesek.

Kockázat:
- regressziók rejtve maradnak
- hozzáférhetőségi szint gyenge
- nehezebb review és karbantartás

Javasolt irány:
- Előbb legyen zöld a lint.
- Ezután kell bevezetni kötelező pre-commit vagy CI check-et.

### 7. Architekturális sodródás és halott kód

Megfigyelés:
- A backendben sok export, típus, helper és teljes modul van, amit a fordító sem lát aktív használatban.
- Ez arra utal, hogy a régebbi irányok és az új orchestrator-alapú megközelítés egymás mellett maradtak.

Kockázat:
- nő a mentális terhelés
- nehezebb eldönteni, melyik út az aktuális
- könnyebben keletkezik duplikált javítás

Javasolt irány:
- Külön cleanup fázis kell.
- Rögzíteni kell a kanonikus trading flow-t és a kanonikus credential flow-t.

## Pozitívumok

- A frontend build átmegy, tehát az alap UI struktúra menthető.
- A store, SSE, dashboard, bots, settings felosztás jó kiindulási alap.
- A backend rétegezése alapvetően értelmes: API, trading, db, middleware, crypto.
- Már most van több fontos domain fogalom: strategies, orchestrator, portfolio, session, order.

## Összegzés

A projekt nem reménytelen, sőt jó alapokon áll, de most még inkább egy aktív építési fázisban lévő rendszer, mint kész trading platform. A legnagyobb nyereséget rövid távon a credential flow rendbetétele, a teszt/lint zöldítése, a multi-user határok megerősítése és a frontend valós idejű állapotlogika stabilizálása adja.
