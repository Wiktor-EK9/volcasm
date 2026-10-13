# Wzorce testowe sprawdzone na sprzęcie

Surowe struktury `VolcaSample_Pattern_Data` (2624 B), zbudowane z pustego
wzorca Vosyra `vosyr/Test3/B1/sequences/sequence01`. Przesłane na Volca
Sample (1. generacja) przez SYRO 2026-10-13, ocena odsłuchem. Wszystkie
części grają slot 0 (krótki perkusyjny sampel). Wyniki opisane w
`docs/vsj-format.md`, sekcja 4.

Offsety części: `32 + 256*i`; StepOn @+2 (u16 LE), Param[] @+9 (11 B),
FuncMemoryPart @+20.

## `t1_mute`: bit 4 FuncMemoryPart

| część | StepOn | FuncMemoryPart |
|---|---|---|
| P1 | `0x1111` (kroki 1, 5, 9, 13) | `0x10` |
| P2 | `0xaaaa` (kroki parzyste) | `0x00` |

Wynik: słychać 4 uderzenia na takt (tylko P1). Bit 4 = 1 oznacza, że część
gra.

## `t2_bit7`: bit 7 w Param[]

| część | StepOn | Param[] |
|---|---|---|
| P1 | `0x1111` | jak Vosyr, PAN = `0x81` (lewo −63) |
| P2 | `0x4444` (kroki 3, 7, 11, 15) | surowe `VolcaSample_Pattern_Init`: `7f 7f 40 00 7f 40 00 7f 00 7f 7f` (PAN = `0x7f`, prawo +63) |

Wynik: naprzemiennie lewo-prawo, równy poziom. Bit 7 nie zmienia działania
parametru (poza SPEED).

## `t3_speed`: tryb SPEED

| część | StepOn | SPEED |
|---|---|---|
| P1 | `0x0101` (kroki 1, 9) | `0xc0` (ciągły, 0) |
| P2 | `0x1010` (kroki 5, 13) | `0x4c` (bez bitu 7: 64 + 12) |
| P3 | `0x0404` (kroki 3, 11) | `0xcc` (ciągły, +12) |

Wynik: P2 o oktawę wyżej niż P1, P3 nieznacznie wyżej. Bit 7 w SPEED
wybiera tryb: 1 = ciągły, 0 = półtony (`64 + półtony`).
