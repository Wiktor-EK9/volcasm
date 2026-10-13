# Format projektu Vosyra (.vsj), ustalony z obserwacji

Dokument clean-room: wszystko poniżej pochodzi z analizy plików i zrzutów
ekranu, bez dostępu do kodu Vosyra. Aktualizuj go po każdym nowym teście.

**Źródła (wszystko z Vosyra na Windows, 2026-10-12):**
- `Test1`: projekt z 25 samplami i 10 sekwencjami, 4 zrzuty ekranu
- `Test2`: sekwencje z wszystkimi funkcjami części (opis w `Test2.txt`)
- `Test3`: seria `B1`..`B8r`, w każdej jedna zmiana względem pustego
  projektu `B1` (opis w `Testy B.txt`)
- Test na sprzęcie (Volca Sample, 2026-10-13): trzy wzorce testowe
  zbudowane z `B1` i przesłane przez SYRO, ocena odsłuchem (sekcja 4)

**Legenda statusów:**
- **POTWIERDZONE**: zgodne ze zrzutami ekranu, testem kontrolowanym, SDK
  i/lub testem na sprzęcie
- **HIPOTEZA**: spójne z danymi, ale bez bezpośredniego dowodu
- **NIEZNANE**: wartości widoczne w pliku, znaczenie nieustalone

Wszystkie wartości wielobajtowe: little endian.

## 1. Układ projektu (katalog, nie jeden plik)

```
Test1/
  Test1.vsj                      65535 B, plik projektu (nagłówek "PRST")
  samples/
    samples                      65535 B, indeks 100 slotów (nagłówek "SMST")
    sample<liczba>               RIFF/WAV bez rozszerzenia (jeden na sampel)
  sequences/
    sequence01 ... sequence10    po 2624 B, surowe struktury SYRO
```

`.vsj` i `samples/samples` mają zawsze 65535 B (stały bufor, reszta zera),
pliki sekwencji zawsze 2624 B. **POTWIERDZONE** (wszystkie projekty).
Pusty projekt (`B1`) ma komplet 10 plików `sequenceNN` (identycznych)
i indeks sampli bez rekordów, katalog `samples/` bez plików WAV.

## 2. `samples/sample<liczba>`

- Zwykły RIFF/WAVE PCM, **16 bit, mono, 31 250 Hz** (natywny format Volci).
  Vosyr zapisuje w projekcie wyłącznie sample już skonwertowane. Oryginałów
  nie ma. **POTWIERDZONE** (`file`, moduł `wave`, wszystkie pliki z Test1).
- Nazwa pliku: `sample` + liczba. Liczba to czas w milisekundach od epoki
  (np. `1791829490708` = 2026-10-12 18:24:50 UTC, zgodnie z godziną na
  zrzutach). **HIPOTEZA** (potwierdzone tylko zgodnością dat).
- Suma rozmiarów plików w `Test1` = 3 595 158 B, UI pokazuje "MEMORY:
  3.60MB" (MB dziesiętne). Suma ramek = 1 797 029 (57,5049 s, sprawdzone
  loaderem i modułem `wave` Pythona), UI pokazuje "57.51S". **POTWIERDZONE**
  (suma). Zwykłe zaokrąglenie dałoby 57.50, więc Vosyr najpewniej zaokrągla
  w górę. **HIPOTEZA** (jeden punkt danych). (Korekta 2026-10-13: wcześniej
  zapisano sumę jako 57,505 s.)
- Dodanie sampla nie zmienia pliku `.vsj` (patrz sekcja 5, B6-B8r).

## 3. `samples/samples` (indeks slotów)

```
offset 0     4 B       "SMST"
offset 4     100 x 215 rekordy slotów 0..99
offset 21504 4 B       "SMED"
dalej        zera do 65535
```

Rekord (215 B):

| offset | rozmiar | pole | status |
|---|---|---|---|
| +0 | 100 | nazwa wyświetlana, NUL-padded (zapis mieszaną wielkością liter, UI pokazuje WERSALIKI) | POTWIERDZONE |
| +100 | 1 | `1` = slot zajęty, `0` = pusty | POTWIERDZONE |
| +101 | 10 | zera we wszystkich testach | NIEZNANE |
| +111 | 100 | nazwa pliku w `samples/` (np. `sample1791829490708`), NUL-padded | POTWIERDZONE |
| +211 | 4 | zera we wszystkich testach | NIEZNANE |

Numer slotu = kolejność rekordu (rekord n zaczyna się @ 4 + 215*n).

Potwierdzone testami `Test3`:
- sampel w slocie 0 (`B6`) i w slocie 5 (`B7`) zmienia wyłącznie rekord
  danego slotu: nazwa, flaga `1`, nazwa pliku;
- zmiana nazwy (`B7r`, "Renamed123") zmienia tylko pole nazwy (+0),
  nazwa pliku i plik WAV zostają;
- nazwa nowego sampla pochodzi z nazwy zaimportowanego pliku
  (`Samp1`, `Samp3`) **HIPOTEZA** (nazwy źródłowych plików nie były podane);
- usunięcie sampla (`B8r`) zeruje cały rekord slotu (indeks wraca do stanu
  jak dla pustego slotu) i usuwa plik WAV z `samples/`.
- katalog `B7r/samples/` zawiera także plik `sample1791835640753` (sampel
  `Samp1` z `B8`), którego indeks nie wymienia. Skąd się wziął, nie wiadomo
  (być może kolejność zapisów przy testach). **NIEZNANE.** Loader czyta
  tylko pliki wskazane w indeksie, a resztę pomija.

## 4. `sequences/sequenceNN` (NN = 01..10)

Plik to **surowa struktura `VolcaSample_Pattern_Data` z SDK Korga**
(2624 B = 0xA40), czyli to, co SDK przyjmuje jako dane typu pattern
(numer sekwencji 0-9 w SDK odpowiada plikom 01-10).

Nagłówek i stopka zgodne z SDK **(POTWIERDZONE)**: `Header` = 0x54535450
(bajty `PTST`) @0, `DevCode` = 0x33b8 @4, `Footer` = 0x44455450 (bajty
`PTED`) @2620.

| offset | rozmiar | pole | status |
|---|---|---|---|
| 0 | 4 | Header 0x54535450 | POTWIERDZONE |
| 4 | 2 | DevCode 0x33b8 | POTWIERDZONE |
| 6 | 2 | `Reserved[2]` w SDK; `ff 00` we wszystkich plikach, tak samo w presetach Korga | POTWIERDZONE (nazwa z SDK), znaczenie NIEZNANE |
| 8 | 2 | **ActiveStep**, bit n = krok n+1, domyślnie 0xffff | POTWIERDZONE |
| 10 | 22 | `Padding1[0x16]` w SDK; głównie 0xff z kilkoma wyzerowanymi bitami, bajt w bajt jak w presetach Korga (`pattern/preset_pattern_*.dat`). `VolcaSample_Pattern_Init` wpisuje tu zera | POTWIERDZONE (wartości fabryczne), znaczenie NIEZNANE |
| 32 + 256*i | 256 | część P(i+1), i = 0..9 | POTWIERDZONE (odstęp 256) |
| 2592 | 28 | `Padding2[0x1c]` w SDK | POTWIERDZONE (SDK) |
| 2620 | 4 | Footer | POTWIERDZONE |

ActiveStep potwierdzony w `Test2`: S2 z wyłączonymi krokami 1 i 16 = 0x7ffe,
S3 z wyłączonym krokiem 8 = 0xff7f. W `Test1` sekwencja 1 = 0xefff (krok 13).

**Część domyślna** (pusty projekt `B1`, wszystkie 10 części identyczne):
SampleNum 0, StepOn 0, Accent 0, Level 127, Param[] =
`ff c0 c0 80 ff c0 80 ff 80 ff ff`, FuncMemoryPart `0x10`, reszta zera.

Część (256 B), offsety względem początku części:

| +off | rozmiar | pole | status |
|---|---|---|---|
| +0 | 2 | SampleNum (numer slotu 0-99) | POTWIERDZONE (Test1 S1, Test2 P9 = 24) |
| +2 | 2 | StepOn, bit n = krok n+1 | POTWIERDZONE (Test1 S4 P1-P4) |
| +4 | 2 | Accent = 0 | zgodne z SDK |
| +6 | 2 | `Reserved` w SDK, zera we wszystkich testach | POTWIERDZONE (SDK) |
| +8 | 1 | Level = 127 (SDK: musi być 127) | zgodne z SDK |
| +9 | 11 | Param[0..10], patrz niżej | POTWIERDZONE |
| +20 | 1 | FuncMemoryPart, patrz niżej | POTWIERDZONE |
| +21 | 11 | `Padding1[11]` w SDK, zera we wszystkich testach | POTWIERDZONE (SDK) |
| +32 | 224 | Motion[14][16], patrz niżej | POTWIERDZONE (układ i wiersze, SDK) |

### Param[] (11 bajtów @ +9)

Kolejność: LEVEL, PAN, SPEED, AMP EG ATTACK, AMP EG DECAY, PITCH EG INT,
PITCH EG ATTACK, PITCH EG DECAY, START POINT, LENGTH, HI CUT.

Kodowanie bajtu (z UI), **POTWIERDZONE**:
- LEVEL, AMP EG ATTACK/DECAY, PITCH EG ATTACK/DECAY, START POINT, LENGTH,
  HI CUT: `bajt = 128 + wartość`, wartość 0..127 (bajty 128..255)
- PAN, PITCH EG INT: `bajt = 192 + wartość`, wartość −63..+63 (bajty 129..255)
- SPEED (tryb ciągły): `bajt = 192 + wartość`, wartość −63..+63 (129..255)
- SPEED w trybie półtonów: `bajt = 64 + półtony` (bit 7 = 0), patrz niżej.
  Kodowanie w plikach Vosyra nieobserwowane (test 9 w sekcji 6)

Skrajne wartości potwierdzone w `Test2` (P9): PAN −63 -> 129, SPEED +63 -> 255,
PITCH EG INT −63 -> 129, PITCH EG ATTACK 127 -> 255, PITCH EG DECAY 0 -> 128.
Pełne zestawy wartości zgodne ze zrzutami: Test1 S1 P1 i S4 P8 (22/22).

**Przesunięcie +128 (bit 7), ustalone na etapie 2:** README SDK podaje
zakresy 0-127 (PAN i PITCH EG INT 1-127), a dla SPEED dwa zakresy: 40-88
(półtony) i 129-255 (tryb ciągły, 192 = środek). `VolcaSample_Pattern_Init`
wpisuje surowe wartości (`7f 40 40 00 7f 40 00 7f 00 7f 7f`). Jednak fabryczne wzorce Korga
(`pattern/preset_pattern_01..10.dat` w SDK) mają bit 7 ustawiony tak samo
jak Vosyr (np. `7f ff c0 af 80 ff ...`). Domyślna część Vosyra to dokładnie
`Init` + 0x80 na każdym bajcie `Param[]`. Kodowanie +128 jest więc zgodne
z danymi z urządzenia. **POTWIERDZONE** (presety Korga).

**Bit 7 na sprzęcie, POTWIERDZONE (test 2026-10-13, wzorzec `t2_bit7`):**
część z surowymi wartościami `Init` (bez bitu 7) i PAN = `0x7f` grała
w prawym kanale, z normalnym poziomem, naprzemiennie z częścią zapisaną
jak w Vosyrze (PAN = `0x81`, lewy kanał). Dla parametrów innych niż SPEED
bit 7 nie zmienia działania, a wartość niosą bity 0-6. Przy zapisie
trzymamy się kodowania z bitem 7 (Vosyr, presety Korga).

**SPEED, POTWIERDZONE (test 2026-10-13, wzorzec `t3_speed`):** bit 7
wybiera tryb.
- bit 7 = 1: tryb ciągły, `bajt = 192 + wartość` (−63..+63);
- bit 7 = 0: tryb półtonów, `bajt = 64 + półtony`. `0x4c` (+12) zabrzmiało
  oktawę wyżej niż `0xc0`, a `0xcc` (ciągły +12) tylko trochę wyżej.
Oba tryby i ich zakresy (40..88 dla półtonów, 129..255 dla trybu ciągłego)
opisuje README SDK (sekcja o `Param[]`), sprzęt to potwierdził. Skrajne
wartości nietestowane. (Korekta 2026-10-13: wcześniej błędnie zapisano, że
README podaje dla SPEED tylko 0-127.) Presety Korga używają obu trybów (np. preset 01, część 3:
`0x40`).

### FuncMemoryPart (1 bajt @ +20), POTWIERDZONE w Test2

| bit | znaczenie | obserwacja |
|---|---|---|
| 0 | MOTION włączony | P1: 0x11 |
| 1 | LOOP | P2: 0x12 |
| 2 | REVERB | P3: 0x14 |
| 3 | REVERSE | P4: 0x18 |
| 4 | **1 = część gra**, 0 = wyciszona (MUTE zeruje bit) | domyślnie 0x10, P5 z MUTE: 0x00. README SDK opisuje to wprost: „Mute On/Off (1=mute off)”, choć sama stała nazywa się `VOLCASAMPLE_FUNC_MUTE`. **POTWIERDZONE na sprzęcie** (wzorzec `t1_mute`: słychać tylko część z bitem 4 = 1) |

Flaga MOTION jest niezależna od danych: P7 i P8 mają dane motion przy
FuncMemoryPart = 0x10 (MOTION wyłączony). Bity 5-7 nieobserwowane (zawsze 0).

### Motion[14][16] (224 B @ +32, do końca części)

Bajt = `128 + wartość`, 0 = brak danych w kroku (UI pokazuje "-").
Wyjątek według README SDK: wiersze SPEED (4 i 5) mają surową wartość
pokrętła 0..127, bez +128 („speed: knob value (0~127), all other parameters:
knob value +128, for all parameters, 0=no motion data”). Kodowanie SPEED
w motion w plikach Vosyra nieobserwowane (test 8 w sekcji 6), na sprzęcie
nietestowane.
Adres: `+32 + wiersz*16 + (krok-1)`. Wiersz 14 x 16 kroków = dokładnie 224 B.

Trzy zmierzone punkty, **POTWIERDZONE**:

| test | parametr, krok | offset w części | wiersz, kolumna | bajt |
|---|---|---|---|---|
| P7 | LEVEL 69, krok 9 | +40 | 0, 8 | 197 |
| P8 | AMP EG ATTACK 45, krok 6 | +133 | 6, 5 | 173 |
| P8 | HI CUT 68, krok 14 | +253 | 13, 13 | 196 |

Wiersze **POTWIERDZONE** nazwami stałych w SDK
(`VOLCASAMPLE_MOTION_LEVEL_0` ... `VOLCASAMPLE_MOTION_HICUT`, 0-13),
zgodne z trzema pomiarami i etykietą "LEVEL START/END" w UI Vosyra:

| wiersz | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| parametr | LEVEL start | LEVEL end | PAN start | PAN end | SPEED start | SPEED end | AMP ATK | AMP DEC | PITCH INT | PITCH ATK | PITCH DEC | START | LENGTH | HI CUT |

Wiersze 0, 6, 13 zmierzone w Vosyrze, wszystkie 14 nazwane w `volcasample_pattern.h`.

## 5. `Test1.vsj` (plik projektu)

Nagłówek `PRST` @0, u32 @4 = 4854 i u32 @8 = 251 są identyczne we wszystkich
projektach (Test1, Test2, B1..B8r): prawdopodobnie stałe/wersja. **NIEZNANE.**

### Ustalone (testy B1..B5, **POTWIERDZONE**)

W projektach bez dodatkowej zawartości sekwencji (B1..B5, Test1):

| offset | rozmiar | pole | dowód |
|---|---|---|---|
| 223 | u8 | **tempo (BPM)** | B1 = 125, B2 = 97, B5 = 140 |
| 824 | u8 | **swing** | B3 = 63, Test1 = 14, B1 = 0 |
| 825 | u8 | **reverb** | B4 = 94, Test1 = 45, B1 = 0 |
| 213-222 | 10 x 0x7f | stałe w B1 (znaczenie nieznane, tuż przed tempem) | NIEZNANE |
| 926-1025 | 100 x 0x40 | tablica 100 wartości, domyślnie 64 (np. ustawienie per slot) | HIPOTEZA |

Tempo jest **jedno globalne**, nie per sekwencja (B5: zmiana tempa nie
dotyka żadnego pliku sekwencji). Swing i reverb to także ustawienia globalne
(zmieniają tylko `.vsj`, sekwencje bez zmian). Wartości swing/reverb zapisane
jako 63 i 94, UI Vosyra nie pokazuje liczb przy tych pokrętłach. Zakres
0-127 to **HIPOTEZA** (pokrętło wypełnia się proporcjonalnie, 94 ≈ 3/4 skali).
Uwaga: w `B5` pole reverb nadal ma 94, bo projekt powstał po `B4`.

Sample nie wpływają na `.vsj`: `B6`, `B7`, `B7r`, `B8`, `B8r` mają plik
`.vsj` identyczny z `B1` (zero różnic).

### Nierozstrzygnięte: układ zmienny (NIEZNANE)

**Offsety powyżej nie są stałe.** W `Test2` cały ogon pliku jest przesunięty o
**+8 bajtów**: 10 x 0x7f jest @221-230, tempo @231, tablica 100 x 0x40
@934-1033. Dlatego loader **nie może** czytać `.vsj` ze stałych offsetów, dopóki
nie ustalimy struktury.

Dodatkowe obserwacje:
- `Test2` (sekwencje z motion, P9 z samplem 24 i skrajnymi wartościami,
  flagi części): w obszarze przed 213 pojawiają się teksty ASCII `"0|1"`
  @118-120, bajty `"13"` @122-123 i `"0|1"` @143-145. Pozostałe bajty zero.
- `Test1` (25 sampli, wzorce na S1 i S4, ACTIVE STEP 13 wyłączony): 15 flag
  `01` @13-16, 18, 21, 42-48, 50, 51 oraz `03` @212. Tempo @223 bez
  przesunięcia.
- Pusty `B1` nie ma żadnej z tych zawartości.

Nie wiadomo, która funkcja (motion, numer sampla w części, wzorce, ACTIVE
STEP, flagi) odpowiada za które pola. W `Test2`/`Test1` zmieniono wiele rzeczy
naraz, więc atrybucja jest niemożliwa. Potrzebne testy jednofunkcyjne
(sekcja 6).

Nie znaleziono znacznika końca (w przeciwieństwie do `SMED` i `PTED`).

## 6. Testy kontrolowane

**Wykonane:** pusty projekt, tempo, swing, reverb, sample w slocie 0 i 5,
zmiana nazwy, dodanie i usunięcie sampla (`Test3`), flagi części, ACTIVE
STEP, motion w kilku miejscach (`Test2`).

**Do wykonania (każdy z pustego projektu `B1`, jedna zmiana na zapis):**

Cel A: ustalić, co zmienia układ `.vsj` (flagi w Test1, teksty i przesunięcie
w Test2):
1. Tylko numer sampla w P1 = slot z istniejącym samplem (np. 0).
2. Tylko numer sampla w P1 = pusty slot (np. 24).
3. Tylko włączony MOTION na P1 (bez danych).
4. Tylko dane motion: LEVEL 69 na kroku 9 w P7, MOTION wyłączony.
5. Tylko włączony jeden krok wzorca (P1, krok 1).
6. Tylko ACTIVE STEP 1 wyłączony w S1.
7. Tylko START POINT 5 w P6. (Rozbieżność z `Test2` rozstrzygnięta: plik
   ma w P6 START POINT = `0x85`, SPEED domyślny, więc "SPEED +5" w
   `Test2.txt` to pomyłka opisu.)

Cel B: dokończyć dekodowanie sekwencji:
8. Jeden projekt, jedna część (np. P1) z motion na wszystkich 14 polach
   (LEVEL start/end, PAN start/end, SPEED start/end, AMP ATK, AMP DEC,
   PITCH INT, PITCH ATK, PITCH DEC, START, LENGTH, HI CUT), każde w innym
   kroku 1-14. Zapisz listę "pole -> krok". Wiersze są już potwierdzone
   przez SDK, test tylko dla pewności, że Vosyr zapisuje je tak samo.
9. SPEED w trybie półtonów (przełącznik przy etykiecie SPEED): P1 = +5,
   P2 = −5. Sprzęt już potwierdził kodowanie (`64 + półtony`); test sprawdzi,
   czy Vosyr zapisuje tak samo.
10. Wzorzec z włączoną akcentem, jeśli Vosyr ma Accent.

Cel C: zakresy ustawień globalnych:
11. Swing na minimum, potem na maksimum (osobne zapisy).
12. Reverb na minimum, potem na maksimum.
13. Tempo na minimum i maksimum, jakie pozwala ustawić Vosyr.

Wskazówka: do każdego zapisu dołącz krótką listę tego, co ustawiono, i
zrzut ekranu, jak dotąd. Zrzuty pozwalają zweryfikować dekodowanie.

## 7. Zasady dla fixtures w repo

- Nie commituj sampli osób trzecich (nazwy `GS_BLU_*` w `Test1` wyglądają
  na paczkę sampli z zewnątrz). W testach używaj małych, syntetycznych WAV-ów.
- Pliki `sequences/*` i `*.vsj` z własnych projektów można trzymać w
  `tests/fixtures/` (np. `Test1`, `Test2`, `B1`..`B8r`).
