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
and glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 or newer).
If it does not start, install `libfuse2` (or `fuse2`), or run it with
`--appimage-extract-and-run`.

### Windows (10 or 11, 64-bit)

1. Unpack the zip.
2. Run `volcasm.exe`. The build is not signed, so SmartScreen may warn about
   an unknown publisher: click "More info" → "Run anyway".

### macOS (11 or newer, Intel or Apple Silicon)

1. Unpack the zip and move `Volcasm.app` to Applications.
2. The build is not signed by Apple, so the first start is blocked: right-click
   the app → Open → Open. If macOS still refuses, run
   `xattr -dr com.apple.quarantine /Applications/Volcasm.app` in Terminal.

Transfers on macOS have not been tested yet.

## Updates

At start Volcasm asks GitHub once whether a newer version exists. If there is
one, "UPDATE AVAILABLE" appears in red in the bottom right corner, and the
menu (click the logo) has a button to download it. On Linux (AppImage) and
Windows the new version replaces the program file; then "Restart to update"
starts it. This needs write access to the folder with the program. On macOS
the button opens the download page. To stop the check, set "Check for
updates at start" to OFF in the menu; the same button then checks on
request. Nothing else is sent: only the request and the program version.

## Connecting the volca

Connect the computer's headphone output to SYNC IN on the volca with a
3.5 mm cable. Set the computer's volume to the maximum and close other
programs that play sound. During a transfer the volca shows the progress;
`Err dcod` on the volca means the sound reached it altered (see below).

## The audio output: 44.1 kHz recommended

The transfer signal is modulated data: equalizers, volume levelling and
low-quality resampling can corrupt it. Volcasm checks the output before
sending.

### Linux

If Volcasm reports that PipeWire runs at another rate, run

    pw-metadata -n settings 0 clock.force-rate 44100

and, after the transfer, `pw-metadata -n settings 0 clock.force-rate 0`.

### Windows

If the output runs at another rate (often 48000 Hz), Volcasm converts the
stream to that rate itself and shows a warning. This should work, but it is
not yet confirmed on Windows hardware: if the volca shows `Err dcod`, set
the format to 44.1 kHz as in point 1 and try again.

1. Settings → System → Sound → your output device (e.g. Speakers or
   Headphones) → Output settings → Format: **16 bit, 44100 Hz (CD Quality)**.
   In the older Control Panel: Sound → device → Properties → Advanced →
   Default Format.
2. On the same page turn off **Audio enhancements** (and any "Loudness
   Equalization", Dolby or vendor sound effects) and **Spatial sound**
   (Windows Sonic, Dolby Atmos).
3. Set the device volume to 100%.

Volcasm sees the format (point 1) but cannot see enhancements or the
volume, so please check points 2 and 3 yourself.

## Factory reset

Menu → Volca → "Restore factory samples…" sends KORG's factory samples to
all 100 slots and empties the 10 sequences, as on a new volca. The transfer
takes about 13 minutes; keep the computer awake and the volume unchanged.
"Erase all samples…" empties all slots in about 10 seconds. Both replace
what is on the volca: it cannot be read back, so keep a project with your
own samples.

## Settings and files

Settings are kept in `~/.config/volcasm/` (Linux and macOS) and
`%APPDATA%\volcasm\` (Windows). Projects are single `.volcasm` files.
