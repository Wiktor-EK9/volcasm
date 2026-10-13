# Volca Sample: ustalenia ze sprzętu i SDK

Wyniki testów na Volca Sample (1. generacja) i obserwacje z SDK Korga, które
nie dotyczą formatu Vosyra (ten opisuje `docs/vsj-format.md`). Statusy jak
tam: **POTWIERDZONE**, **HIPOTEZA**, **NIEZNANE**.

## 1. Pamięć sampli

### Model (POTWIERDZONE, test 2026-10-13)

- Pamięć jest podzielona na bloki po **4096 B**. Sampel zajmuje
  `ceil(bajty / 4096)` bloków, gdzie bajty = 2 × liczba ramek (16 bit mono).
  Innych kosztów na sampel nie ma.
- Na sample jest **1019 bloków** (4 173 824 B), może 1020 (patrz niżej).
- Kompresja SYRO nie zmienia zajętości (README SDK, test wykonany na
  strumieniach skompresowanych do 16 bit).

### Test

Strumienie przygotowane z `syro_wav`, wszystkie z kompresją 16 bit
(bezstratną, patrz sekcja 2). Przed każdym eksperymentem cała pamięć
została skasowana strumieniem `a:` z pustym `all_sample_empty.alldata`
(wyświetlacz: `data`/`ALL`, na koniec `End`).

| eksperyment | wypełniacze | próbki (po 2000 B, 1 blok) | wynik |
|---|---|---|---|
| 1 | sloty 0-39, po 102 350 B (25 × 4094 B) | od slotu 40 | wypełniacze bez błędu, ostatni widoczny slot `S.059` |
| 2 | sloty 0-38, po 102 352 B (25 × 4094 + 2 B) | od slotu 39 | wypełniacze bez błędu, ostatni widoczny slot `S.083` |

Po teście przywrócono fabryczne sample (`a` z `all_sample_preset.alldata`).

Interpretacja:
- Wypełniacze w obu eksperymentach zajmują po 25 bloków, więc mieści się
  w nich 4096 B danych na blok, a nie 4094 B (rozmiar porcji, na które SYRO
  dzieli sampel). Przy porcjach 4094 B wypełniacz z eksperymentu 2 zająłby
  26 bloków, a różnica między eksperymentami wyniosłaby −14 próbek zamiast
  +24/+25.
- Eksperyment 1: 1000 bloków wypełniaczy + 19 lub 20 próbek. Eksperyment 2:
  975 bloków + 44 lub 45 próbek. Oba dają tę samą pojemność, **1019 lub
  1020 bloków**, bez kosztu na sampel. Model z kosztem 1 bloku na sampel
  przewiduje różnicę 13 próbek, a model bajtowy (bez bloków) rozbieżne
  pojemności. Oba odrzucone.
- Nie wiadomo, czy ostatni widoczny slot to ostatni zapisany, czy pierwszy
  odrzucony (komunikatu `Err FuLL` nie zaobserwowano). Przyjmujemy ostrożnie
  1019 bloków. Rozstrzygnięcie: po podobnym teście sprawdzić na Volce, czy
  slot z ostatniego widocznego numeru gra. **NIEZNANE.**

### Pliki `.alldata` z SDK (HIPOTEZA)

- Pierwsze 16 384 B to dwie kopie tablicy bloków (`SATS`…`SATE` @0 i
  @4096) i dwie kopie informacji o samplach (`SIFS`…`SIFE` @8192 i
  @12288). Dalej dane sampli w blokach po 4096 B.
- u16 @6 wygląda na licznik wolnych bloków: 1016 w pustym zrzucie,
  68 w fabrycznym, który ma 956 bloków danych (68 + 956 = 1024).
- Sprzęt przyjął jednak 1019-1020 bloków po skasowaniu pustym zrzutem
  (licznik 1016). Rozbieżność niewyjaśniona. Do liczenia budżetu używamy
  wyłącznie wyniku pomiaru.

## 2. Strumienie SYRO

- Kompresja przy 16 bitach jest bezstratna: `korg_syro_comp.c` kwantyzuje
  tylko przy `bitlen < 16`. Strumień jest krótszy (ok. 1,6×). **POTWIERDZONE**
  (kod SDK; sample po teście pamięci działały).
- Czasy strumieni przy 44,1 kHz: przywrócenie fabrycznych sampli 1243,6 s bez
  kompresji, 795 s (13,3 min) z kompresją 16 bit. Skasowanie całej pamięci
  (pusty `.alldata`) 9,6 s. **POTWIERDZONE** (długość plików).
- Tor audio użyty we wszystkich testach: `pw-play`, PipeWire wymuszony na
  44,1 kHz (`pw-metadata -n settings 0 clock.force-rate 44100`), głośność
  100%.

## 3. Tor audio przez PipeWire (etap 5)

Obserwacja `pw-dump` podczas `pw-play` pliku 44,1 kHz bez wymuszenia
częstotliwości (2026-10-13), **POTWIERDZONE**:
- węzeł strumienia ma `Format` 44 100 Hz S16LE i `node.driver-id`
  wskazujący wyjście karty (`Audio/Sink`);
- wyjście karty ma `Format` 48 000 Hz S32LE, czyli PipeWire przelicza
  częstotliwość po cichu;
- `settings`: `clock.rate` 48000, `clock.allowed-rates` `[ 48000 ]`,
  `clock.force-rate` 0. Graf nie przełączy się sam na 44,1 kHz.
- `Props` węzłów: `channelVolumes` (głośność całkowita), `softVolumes`
  (część programowa, skaluje próbki), `mute`, `softMute`.

### Wyjście słuchawkowe laptopa obsługuje tylko 48 kHz (POTWIERDZONE)

Laptop deweloperski (ASUS Zenbook S13 UX5304MA, Meteor Lake, SOF
`sof-hda-dsp`, kodek Realtek ALC294), wyjście Headphones, pomiar
2026-10-13 przy `clock.force-rate 44100` i odtwarzaniu przez `pw-play`:
- `/proc/asound/card0/pcm0p/sub0/hw_params`: `rate: 48000`, `S32_LE`;
- `EnumFormat` węzła: jedyna częstotliwość 48000;
- `pw-top`: graf (kolumna RATE) 44100, format karty `S32LE 2 48000`.

`clock.force-rate` zmienia tylko zegar grafu PipeWire, a węzeł karty i tak
przelicza na 48 kHz. Kolumna RATE w `pw-top` to częstotliwość grafu, nie
sprzętu. Wniosek: **wszystkie dotychczasowe transfery (etap 3, test
pamięci, przywrócenie fabrycznych sampli, ok. 40 min strumieni) przeszły
przez resampler PipeWire** i Volca dekodowała je bez błędów. Konwersja
częstotliwości PipeWire nie psuje więc strumienia SYRO na tym sprzęcie.
Wyjścia HDMI tego laptopa deklarują zakres 32-192 kHz (nietestowane).

### Jak `volcasm-audio` to sprawdza

1. Strumień `cpal` (backend PipeWire, i16, stereo, 44 100 Hz) startuje od
   ciszy. Nasz węzeł rozpoznajemy po `node.group` = `cpal-<pid>`, a
   urządzenie po `node.driver-id`. Urządzenie jest znane dopiero po
   podłączeniu strumienia, więc wszystkie kontrole odbywają się w czasie
   ciszy (do 3 s). Dane idą dopiero po pozytywnej weryfikacji.
2. Strumień musi mieć format 44 100 Hz stereo, nic nie może być wyciszone.
3. Wyjście karty (`Format`) z 44 100 Hz: OK.
4. Wyjście z inną częstotliwością:
   - urządzenie nie obsługuje 44 100 Hz (`EnumFormat`): ostrzeżenie
     o resamplingu i transfer;
   - urządzenie obsługuje 44 100 Hz (albo formaty nieznane): odmowa. Jeśli
     zegar nie może pracować z 44 100 Hz (`clock.force-rate`,
     `clock.allowed-rates`), od razu z komendą do uruchomienia. W przeciwnym
     razie dopiero po 3 s, bo graf może się jeszcze przełączyć.
5. `softVolumes` ≠ 1 oraz głośność wyjścia < 100% dają ostrzeżenie.
6. Po danych ponowna kontrola grafu. Zmiana częstotliwości w trakcie albo
   niedobór bufora (xrun) jest zgłaszany jako błąd lub ostrzeżenie.

Test na sprzęcie (2026-10-13, `play_syro`, wyjście Headphones,
`clock.force-rate 0`), **POTWIERDZONE**: transfer sampla do slotu 0 i
wzorca `t1_mute` przeszedł z ostrzeżeniem o resamplingu 44,1 → 48 kHz;
sekwencja 1 grała 4 piknięcia na takt.
Wymuszenie 44,1 kHz nie jest więc potrzebne na tym laptopie. Przerwanie w
połowie (Enter) zakończyło program poprawnie (`cancelled`), a Volca
pokazała błąd dekodowania, zgodnie z oczekiwaniem.

Sprawdzone bez Volci: odmowa przy wyjściu 48 kHz z nieznanymi formatami
(wcześniejsza wersja kontroli), ostrzeżenie i transfer ciszy na wyjściu
Headphones przy `clock.force-rate 0`. `play_syro --status` pokazuje
ustawienia zegara i częstotliwości obsługiwane przez wyjścia.
