# Changelog

## [0.2.1]

- Transfer quality: the transfer panel has a Quality list, from 16 bit
  (lossless, the default) down to 8 bit. Below 16 bits samples are
  quantized and the transfer is shorter, about half as long at 8 bits. The
  choice is remembered. Restoring factory samples and erasing stay at
  16 bits.

## [0.2.0]

First public release.

Volcasm manages the samples and sequences of a KORG volca sample (first
generation) and sends them through the computer's audio output to SYNC IN.

- 100-slot sample library with a memory meter. Import of WAV, AIFF, FLAC,
  MP3, Ogg Vorbis and M4A/AAC, converted to the volca's format.
- Sample editing: waveform, start/length, normalize, pitch.
- Recording from an audio input into a slot.
- Editor for the 10 sequences: steps, part parameters and flags, motion
  data.
- Preview of samples and sequences on the computer.
- Transfer of selected slots and sequences, with checks of the audio
  output. Restore factory samples and erase all samples.
- Projects in one `.volcasm` file; import of Vosyr projects (`.vsj`).
- Optional update check at start; in-place update on Linux and Windows.
- Builds for Linux (AppImage), Windows (zip) and macOS (universal .app).

### Changes since the 0.1.1 test build

- Windows: transfers work when the output runs at another rate than
  44.1 kHz (e.g. 48 kHz). Volcasm converts the stream itself and shows a
  warning instead of refusing.
- The transfer dialog reminds you to set the volume to 100% and to turn
  off sound effects and (Windows) spatial sound.
- Restore factory samples and erase all samples (Menu → Volca).
- First-start window: theme (dark, light, system), memory unit, update
  check.
- Import of MP3 files with large ID3 tags (cover art), M4A and AAC.
- The menu stays open after changing settings.
- Fix: whole-memory transfers (restore factory samples, erase all samples)
  could end with 16 KiB of uninitialized memory instead of zeros; a bug in
  KORG's SDK.
- Builds include the licences of all bundled Rust crates
  (`THIRD-PARTY-LICENSES.html`).
