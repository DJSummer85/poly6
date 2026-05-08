# Polymarket Trading Bot - Frontend UI/UX Terv

Ez a dokumentum a Polymarket Trading Bot frontendjének részletes architekturális és UI/UX terve, amely a meglévő Rust backend képességeire (`auth`, `bots`, `market`, `monitoring`, `orders`, `positions`, `settings`, `sse`, `binance`) épül. 

## 1. Technológiai Stack
A legújabb és legmodernebb eszközöket használjuk a maximális teljesítmény, reszponzivitás és fejlesztői élmény érdekében:

*   **Keretrendszer:** Next.js 16.2.4 (App Router, Server Components & Server Actions a leggyorsabb betöltésért).
*   **Nyelv:** TypeScript (Strict mode, teljes típusbiztonság a backend API-val szinkronban).
*   **Kinézet & Formázás:** Tailwind CSS + Shadcn/UI (Radix primitives). Letisztult, "glassmorphism" dizájn, sötét téma (Dark Mode) alapértelmezetten.
*   **Animációk:** Framer Motion (mikro-interakciók, finom oldalátmenetek, hogy a UI "éljen").
*   **Állapotkezelés & Adatlekérés:** Zustand (globális UI state) és SWR / React Query + natív Server-Sent Events (SSE) a valós idejű adatokhoz (árak, bot státuszok).
*   **Validáció:** Zod (űrlapok és API válaszok validálása).
*   **Ikonok:** Lucide React.

## 2. Dizájn Irányelvek és Ergonómia
Mivel egy professzionális kereskedő platformról van szó, a UI-nak egy extrém gyors, prémium "Command Center" (Irányítóközpont) érzetét kell keltenie. A design fókuszában a vizuális tisztaság és a villámgyors adatfeldolgozás áll.

*   **Színvilág és Látványvilág:** Mély, éjfekete hátterek (`bg-black` vagy `bg-zinc-950`) finom üveghatással (glassmorphism) és elmosott neon fényekkel a háttérben. Az akcentus színek legyenek vibrálóak: neon zöld a profithoz és nyerő pozíciókhoz, mélyvörös a veszteséghez/vészleállításhoz, és elektromos kék az aktív elemekhez.
*   **Tipográfia és Adatmegjelenítés:** Modern, geometriai sans-serif betűtípusok (pl. Outfit vagy Inter) a UI elemekhez, és szigorúan Monospace (pl. JetBrains Mono) a másodpercenként frissülő árakhoz, logokhoz és az élő "Beat Price"-hoz, hogy a számok ne ugráljanak.
*   **Animációk és Vizuális Visszajelzés:** Framer Motion segítségével zökkenőmentes kártya-kiemelések, finom szín-átmenetek (zöld/piros felvillanások) az árak változásakor. A terminál-szerű logok sötét, hacker-stílusú ablakban fussanak, folyamatos, sima görgetéssel.
*   **Reszponzivitás:** 
    *   *Desktop:* Sokoszlopos, sűrű elrendezés (könnyen átrendezhető widgetekkel), maximalizálva a képernyőterületet a TradingView chartnak és az élő adatoknak.
    *   *Mobil:* Kártya-alapú nézetek, azonnal elérhető vészleállító gombbal.

---

## 3. Oldalstruktúra és Navigáció

Az alkalmazás két fő részre oszlik: Publikus (Auth) és Privát (Dashboard).

### 3.1. Publikus Oldalak
*   **`/login`**: Bejelentkező oldal.
*   **`/register`**: Regisztrációs oldal (ha engedélyezett).
*   *Dizájn:* Középre igazított, letisztult, "glassmorphism" kártya, finom háttér-animációval.

### 3.2. Privát Oldalak (Siderbar/Top Navigation)
Az oldalsó navigációs sáv (Sidebar) tartalmazza a főbb menüpontokat:

1.  **Dashboard (Command Center)** - `/dashboard`
2.  **Botok Kezelése** - `/bots`
3.  **Piacok & Pozíciók** - `/markets`
4.  **Rendelési Előzmények** - `/orders`
5.  **Beállítások & API Kulcsok** - `/settings`

---

## 4. Képernyők Részletes Terve

### 4.1. Command Center (Dashboard - `/dashboard`)
Ez a központi nézet, amit a felhasználó bejelentkezés után meglát. Elsődleges célja a hipergyors adatmegjelenítés és a mély vizuális fókusz. 

*   **Bot Választó és Fókusz Nézet (Bot Selector):**
    *   Egy elegáns, vizuális lista vagy legördülő menü (Dropdown), ahol a felhasználó kiválaszthatja, hogy a 16 bot közül épp melyiket akarja betölteni.
    *   A kiválasztott bot minden adata (élő terminál logok, futási állapot, egyedi PnL) azonnal a fő képernyőre ugrik.
*   **Alapértelmezett Piac és Binance Élő Kapcsolat (Max Sebesség):**
    *   A dashboard alapértelmezetten a **Bitcoin 5-perces Up/Down piacot** tölti be. Ennek a betöltése abszolút prioritást élvez.
    *   **Binance Websocket:** Közvetlen, folyamatos, milliszekundum pontosságú Binance BTC árfolyam feed. Hatalmas, ragyogó Monospace számokkal a képernyő kiemelt pontján.
*   **TradingView Integráció:**
    *   A képernyő központi elemét egy professzionális, testreszabott TradingView chart foglalja el. Gyertyadiagram, technikai indikátorok, ahol vizuálisan követhető, hogy pontosan mi történik az árfolyammal a bot döntéseihez képest.
*   **"Beat Price" és a Nyerési Feltétel (Up Win) Szekció:**
    *   Egy lenyűgöző, dedikált UI blokk, ami a nyerési feltételt mutatja.
    *   Dinamikusan, egyértelműen jelzi, hogy az "Up" iránynak mi a célára (Beat Price).
    *   Egy elegáns folyamatjelző (progress bar), dinamikus százalékos távolság mutató, és fénylő neon zöld effektusok jelzik, ahogy a BTC közeledik a célhoz.
*   **Felső Statisztikai Sáv és Összesítő:**
    *   Összesített PnL (Zöld/Piros), USDC és MATIC egyenlegek vékony, üveghatású kártyákon.
    *   A maradék botok mini-státusza (fut/áll) egy alsó vagy oldalsó tömörített gridben jelenik meg.

### 4.2. Botok Kezelése (`/bots`)
Részletes nézet az egyes botok konfigurálásához.

*   **Bot Lista / Táblázat:** 
    *   Rendezhető és szűrhető táblázat a botokról (Win rate, EV, Kockázattal korrigált PnL).
*   **Bot Létrehozása / Szerkesztése (Modal vagy Külön oldal):**
    *   Stratégia kiválasztása (Dropdown).
    *   Paraméterek beállítása: Volatilitási küszöb, BTC megerősítés (igen/nem), Belépési és kilépési határok (Entry bounds).
    *   Tét méretezés (Kockázatkezelés).
*   **Egyedi Bot Analitika (`/bots/[id]`):**
    *   Adott bot részletes tranzakciói.
    *   Valós idejű log stream (terminál-szerű fekete doboz a backend logoknak).

### 4.3. Piacok & Pozíciók (`/markets`)
A backend `market.rs` és `positions.rs` adatait jeleníti meg.

*   **Nyitott Pozíciók Táblázat:**
    *   Piac neve, Irány (Yes/No), Átlagos bekerülési ár, Jelenlegi ár, Nem realizált PnL.
    *   Gomb: "Zárás azonnal" (Emergency close).
*   **Piac Kereső / Felfedező:**
    *   Polymarket események keresése, likviditás és aktuális esélyek megjelenítése.

### 4.4. Rendelések (`/orders`)
A backend `orders.rs` végpontjait használja.

*   **Nyitott Rendelések (Open Orders):**
    *   Függőben lévő limit orderek. Gomb a törléshez (Cancel).
*   **Rendelési Előzmények (History):**
    *   Teljesített és törölt rendelések listája lapozható (pagination) formában.

### 4.5. Beállítások és Biztonság (`/settings`)
A rendszer legérzékenyebb része (`settings.rs`, `auth.rs`, `binance.rs`).

*   **API Kulcs Kezelő (Key Management):**
    *   *Polymarket:* API Key, Secret, Passphrase megadása. (Jelszó mezők, szem ikonnal a felfedéshez).
    *   *Binance:* API Key és Secret a fedezeti (hedging) vagy adat funkciókhoz.
    *   *Kulcsok ellenőrzése:* Gomb, amivel a frontend ráüt a backendre, hogy validálja a kulcsokat (Check Connection), és vizuális visszajelzést ad (Zöld pipa vagy Piros hiba).
*   **Globális Kockázatkezelés:**
    *   Napi maximális veszteség limit (Global Stop-Loss).
    *   **EMERGENCY STOP (Vészleállító Gomb):** Egy nagy piros gomb, amely azonnal leállítja az összes botot és opcionálisan törli az összes nyitott rendelést.
*   **Profil / Biztonság:**
    *   Jelszóváltoztatás, 2FA (ha a backend támogatja).

---

## 5. Kiemelt Fejlesztési Irányelvek (Best Practices)

1.  **Adatfolyam (SSE) Optimalizáció:** Az élő árak és a "price to beat" adatok villogásának elkerülése érdekében a Zustand/React állapotokat finoman kell frissíteni (pl. korábbi érték megtartása amíg az új megérkezik, CSS tranzíciók az értékváltozásokra - pl. zöld/piros felvillanás, amikor az ár változik).
2.  **Hibaüzenetek és Toast Notification-ök:** Minden API hívás (főleg a botok indítása/leállítása és a beállítások mentése) azonnali vizuális visszajelzést kell, hogy adjon a jobb alsó sarokban (Shadcn Toaster).
3.  **Skeleton Loaderek:** Amíg az adatok töltenek, pörgő ikonok helyett Skeleton UI elemeket kell használni a jobb UX érdekében.
4.  **Típusok (Types):** Külön `src/types/index.ts` fájlban deklarálni kell az összes backend interface-t (Bot, Position, Order, Market, Status), hogy a frontend és backend teljesen szinkronban legyen.
5.  **Komponens Bázis:** Apró, újrafelhasználható komponensek (pl. `StatCard`, `BotStatusBadge`, `PriceTicker`), melyek Server Component-ként indulnak, és csak a szükséges esetben lesznek Client Component-ek (`"use client"`).

## 6. Összegzés a megvalósításhoz
Ez a terv biztosítja, hogy a 16 bot kezelése ne váljon kaotikussá. A Unified Dashboard megszünteti a dupla menüs rendszert, a Next.js biztosítja az azonnali betöltést, a Framer Motion pedig azt a prémium "WOW" faktort, ami egy modern trading app-tól elvárható.
