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

## 5. Plik `.vsj` (ustawienia projektu)

Układ rozpoznany na 25 projektach (`Test1`, `Test2`, `B1`..`B8r`,
`C1`..`C14`, 2026-10-13). W każdym z nich każdy bajt należy do pola z
tabeli, a pozostałe są zerami. **POTWIERDZONE** (układ i wartości tempa,
swingu, reverbu); znaczenie pól UI to **HIPOTEZA**.

```
offset  rozmiar  pole
0       4        "PRST"
4       4        u32 4854, we wszystkich plikach               NIEZNANE (stała/wersja)
8       4        u32 251, we wszystkich plikach                NIEZNANE
12      100      bajt na (sekwencja, część): 12 + 10*s + p     HIPOTEZA: stan UI
112     zmienny  100 napisów ASCII zakończonych NUL, w tej
                 samej kolejności (s, p); pusty = sam NUL       HIPOTEZA: stan UI
t       1        sekwencja widoczna przy zapisie (0 = S1)       HIPOTEZA (mocna)
t+1     10       0x7f we wszystkich plikach                     NIEZNANE
t+11    1        tempo (BPM)                                    POTWIERDZONE
t+612   1        swing                                          POTWIERDZONE
t+613   1        reverb                                         POTWIERDZONE
t+714   100      0x40 we wszystkich plikach                     NIEZNANE
```

`t` = koniec bloku napisów. Gdy wszystkie są puste, `t` = 212, więc tempo
jest @223, swing @824, reverb @825. To wyjaśnia wcześniejszą obserwację
„przesuniętego ogona”: w `Test2` napisy `"0|1"`, `"13"`, `"0|1"` zajmują
o 8 bajtów więcej, w `C4`/`C5` napis `"0|1"` o 3.

Zakresy (testy C9..C14, autor, Vosyr na Windows):
- tempo 56..240 BPM (`C13` = 56, `C14` = 240), jeden bajt, kolejny bajt
  zawsze 0;
- swing 0..75 (`C9` = 0, `C10` = 75), UI nie pokazuje liczby;
- reverb 0..127 (`C11` = 0, `C12` = 127), UI nie pokazuje liczby.

Pola UI (HIPOTEZY, nieużywane przy imporcie):
- bajt (s, p) @12: `C1` i `C2` (kroki w S1 P1) ustawiają @12 = 1; `C3`
  (włączony MOTION), `C7` (ACTIVE STEP) i `C8` (numer sampla) nie zmieniają
  `.vsj`. W `Test1` flagi stoją przy S1 P2-P5, P7, P10 i S4 P1-P7, P9, P10.
  Znaczenie niejasne (nie „część ma kroki”); prawdopodobnie stan widoku.
- napis (s, p): pojawia się po edycji motion w części: `"0|1"` dla LEVEL
  (wiersze start/end = 0 i 1), `"13"` dla HI CUT (wiersz 13). W `C5` dwie
  wartości LEVEL dają ten sam napis co jedna w `C4`, a `C6` (S2) zapisuje
  go w pozycji S2 P7. Wygląda na wybór wierszy w edytorze motion Vosyra.
- bajt `t`: `C6` (zapis na S2) = 1, `Test1` (ostatni zrzut na S4) = 3.

Sample nie wpływają na `.vsj` (`B6`..`B8r`, `C8`). Tempo jest jedno,
globalne (`B5`).

Import (`volcasm-vosyr-import`, `settings.rs`) czyta wyłącznie tempo, swing
i reverb, idąc po napisach. Jeśli układ się nie zgadza (napis dłuższy niż
64 znaki, nie-ASCII, brak NUL, za krótki plik), zgłasza błąd zamiast
zgadywać, a sample i sekwencje importują się dalej.

## 6. Testy kontrolowane

**Wykonane:** pusty projekt, tempo, swing, reverb, sample w slocie 0 i 5,
zmiana nazwy, dodanie i usunięcie sampla (`Test3`), flagi części, ACTIVE
STEP, motion w kilku miejscach (`Test2`), seria `Test4` (`C1`..`C14`,
opis w `Test4/Zmiany.txt`): kroki, MOTION bez danych, motion w S1 i S2,
ACTIVE STEP, numer sampla, skrajne wartości swing, reverb i tempa.

Wyniki `Test4` dla sekwencji, **POTWIERDZONE**: `C1`/`C2` StepOn S1 P1 =
`0x0001` / `0x0011`; `C3` FuncMemoryPart P1 = `0x11`; `C4` Motion LEVEL
krok 9 P7 = 196 (68 + 128) przy MOTION wyłączonym; `C5` dodatkowo krok 10 =
199 (71); `C6` to samo w S2; `C7` ActiveStep = `0xfffe`; `C8` SampleNum P1
= 5. Uwaga z `Zmiany.txt`: w Vosyrze nie da się ustawić motion dokładnie na
69/70, stąd 68 i 71.

**Do wykonania (opcjonalnie):**
- Motion na wszystkich 14 polach jednej części, każde w innym kroku
  (sprawdzenie, czy Vosyr zapisuje wiersze tak jak SDK, w tym kodowanie
  SPEED w motion, D19).
- SPEED w trybie półtonów: P1 = +5, P2 = −5 (czy Vosyr zapisuje
  `64 + półtony` jak sprzęt).
- Znaczenie bajtów UI (sekcja 5), jeśli kiedyś okaże się potrzebne.

## 7. Zasady dla fixtures w repo

- Nie commituj sampli osób trzecich (nazwy `GS_BLU_*` w `Test1` wyglądają
  na paczkę sampli z zewnątrz). W testach używaj małych, syntetycznych WAV-ów.
- Pliki `sequences/*` i `*.vsj` z własnych projektów można trzymać w
  `tests/fixtures/` (np. `Test1`, `Test2`, `B1`..`B8r`).
