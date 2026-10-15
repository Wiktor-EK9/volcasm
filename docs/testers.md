# Volcasm – test build

Volcasm manages the samples and sequences of a KORG volca sample (first
generation) and sends them to it through the computer's audio output.

This is a test build. Please report what works, what does not, and the exact
messages you see.

## Running it

### Linux (AppImage)

1. Make the file executable: `chmod +x volcasm-*-x86_64.AppImage`
   (or in the file manager: Properties → Allow executing as program).
2. Run it.

Requirements: PipeWire (the default sound system on current distributions)
and glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 or newer).
If it does not start, install `libfuse2` (or `fuse2`), or run it with
`--appimage-extract-and-run`.

### Windows (10 or 11, 64-bit)

1. Unpack the zip.
2. Run `volcasm.exe`. The build is not signed, so SmartScreen may warn about
   an unknown publisher: click "More info" → "Run anyway".

## Connecting the volca

Connect the computer's headphone output to SYNC IN on the volca with a
3.5 mm cable. Set the computer's volume to the maximum and close other
programs that play sound. During a transfer the volca shows the progress;
`Err dcod` on the volca means the sound reached it altered (see below).

## The audio output must run at 44.1 kHz

The transfer signal is modulated data: resampling, equalizers or volume
levelling corrupt it. Volcasm checks the output before sending and refuses
when it would be resampled.

### Linux

If Volcasm reports that PipeWire runs at another rate, run

    pw-metadata -n settings 0 clock.force-rate 44100

and, after the transfer, `pw-metadata -n settings 0 clock.force-rate 0`.

### Windows

1. Settings → System → Sound → your output device (e.g. Speakers or
   Headphones) → Output settings → Format: **16 bit, 44100 Hz (CD Quality)**.
   In the older Control Panel: Sound → device → Properties → Advanced →
   Default Format.
2. On the same page turn off **Audio enhancements** (and any "Loudness
   Equalization", Dolby or vendor sound effects).
3. Set the device volume to 100%.

Volcasm checks the format (point 1) but cannot see enhancements or the
volume, so please check points 2 and 3 yourself.

## Settings and files

Settings are kept in `~/.config/volcasm/` (Linux) and `%APPDATA%\volcasm\`
(Windows). Projects are single `.volcasm` files.
