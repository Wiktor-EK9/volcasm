<div align="center">

<img src="volcasm-app/assets/icon.png" alt="Volcasm logo" width="160">

<h1 align="center">Volcasm</h1>

[![Written in Rust](https://img.shields.io/badge/Written%20in-Rust-red?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Runs on Linux · Windows · Macos](https://img.shields.io/badge/Runs%20on-Linux%20%C2%B7%20Windows%20%C2%B7%20Macos%20(maybe)-red?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/Wiktor-EK9/volcasm)
[![License GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-red?style=for-the-badge)](LICENSE)

</div align="center">

Sample and sequence manager for the first generation KORG Volca Sample, built in Rust.
Volcasm keeps a library of 100 sample slots and 10 sequences on your computer, lets you edit them, and send your sample pack to the volca through the computer's audio output into the volca's SYNC IN jack.

Linux, Windows and macOS *(UNTESTED)*. 

Free software under the GPL-3.0-or-later license.

## Features

- **Sample library:** 100 slots with a memory meter for the volca's ~4 MB.
  Import WAV, AIFF, FLAC, MP3, Ogg Vorbis and M4A/AAC by opening or dropping
  files; they are converted to the volca's format (16-bit mono, 31.25 kHz).
- **Sample editing:** waveform view, start/length trimming, normalize and
  pitch change.
- **Recording:** record from an audio input straight into a slot.
- **Sequences:** edit the 10 sequences: steps, part parameters, part flags
  (motion, loop, reverb, reverse, mute) and motion data.
- **Preview:** play samples and sequences on the computer before sending.
- **Transfer:** send selected slots and sequences, restore KORG's factory
  samples or erase all samples. Before sending, Volcasm checks the audio
  output, because the transfer signal breaks if it is altered on the way.
- **Projects:** everything is saved in one `.volcasm` file. Vosyr projects
  (`.vsj`) can be imported.
- **Updates:** optional check for a new version at start.

*The volca cannot send anything back to the computer: its samples and
sequences cannot be read out. Keep your project file as the backup.*

## Download

Get the latest build from
[Releases](https://github.com/Wiktor-EK9/volcasm/releases).

- **Linux:** `volcasm-*-x86_64.AppImage`.
  Needs PipeWire and glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 or newer).
  If it does not start, install `libfuse2` (or `fuse2`).
  
- **Windows 10/11 (64-bit):** unpack the zip and run `volcasm.exe`.
  
- **macOS 11 or newer:** unpack the zip and move `Volcasm.app` to
  Applications. The app is not signed by Apple: right-click → Open → Open
  on the first start.
  
  *The macOS version has not been tested yet. Feel free to try it and let me know how it goes!*

## Sending to the volca

1. Connect the computer's headphone output to **SYNC IN** on the volca with
   a 3.5 mm cable.
2. Set the output volume to 100% and close other programs that play sound.
3. Turn off equalizers, "audio enhancements", loudness levelling and similar
   effects for that output. On Windows also turn off spatial sound
   (Windows Sonic, Dolby Atmos).
4. Sample rate:
   - Windows: any output rate works; Volcasm converts the stream to it. If
     the volca shows `Err dcod`, set Sound settings → your output → Format
     to **16 bit, 44100 Hz**.
   - Linux: if Volcasm asks for it, run
     `pw-metadata -n settings 0 clock.force-rate 44100`, and
     `pw-metadata -n settings 0 clock.force-rate 0` after the transfer.
5. Press send in Volcasm. The volca shows the progress.

`Err dcod` on the volca means the signal arrived altered: check the points
above and **make sure that any additional sound effects or enhancements such as surround sound are turned off on your computer.** 
More details for testers are in [docs/testers.md](docs/testers.md).

## Building from source

Requirements: Rust (stable), a C compiler and libclang (for the KORG SYRO
library), and on Linux the ALSA and PipeWire development files.

Arch Linux / CachyOS:

```sh
sudo pacman -S --needed rust base-devel clang alsa-lib pipewire
```

Debian / Ubuntu:

```sh
sudo apt install build-essential libclang-dev pkg-config libasound2-dev libpipewire-0.3-dev
```

Then:

```sh
cargo build --release -p volcasm-app   # binary: target/release/volcasm
cargo test --workspace
```

Packaging (`packaging/*.sh`) also needs
[cargo-about](https://github.com/EmbarkStudios/cargo-about)
(`pacman -S cargo-about` or `cargo install cargo-about`).

## Repository layout

| Crate | Purpose |
|---|---|
| `volcasm-syro-sys` | FFI bindings to KORG's SYRO library (vendored SDK) |
| `volcasm-core` | data model, sample processing, SYRO stream encoding, `.volcasm` files |
| `volcasm-audio` | audio output and input, checks of the audio path |
| `volcasm-vosyr-import` | loader for Vosyr `.vsj` projects |
| `volcasm-app` | the GUI (egui / eframe), binary `volcasm` |

`docs/vsj-format.md` describes the Vosyr project format as worked out from
test projects (clean-room: no Vosyr code was used). `docs/hardware.md` has
notes from tests on the device.

## License and credits

Volcasm is licensed under the [GNU GPL v3.0 or later](LICENSE).

- Rust crates: each build includes `THIRD-PARTY-LICENSES.html` with their
  licences (generated by `cargo about`, see `about.toml`).
- KORG Volca Sample SDK (SYRO): BSD-3-Clause, see
  `volcasm-syro-sys/vendor/volcasample/COPYING`. Factory samples and
  patterns come from the same SDK.
- Fonts: Overpass, Barlow Semi Condensed, DSEG7 (SIL OFL 1.1).
- Icons: Phosphor (MIT).
- Inspired by Vosyr (Frederikson Labs) and
  [Volca Sampler](https://github.com/benwiley4000/volca-sampler) by Ben
  Wiley.

Volcasm is not affiliated with or endorsed by KORG. "volca" is a trademark
of KORG Inc.
