#!/bin/sh
# Regenerates the synthetic import fixtures (ffmpeg 9.0). Each file is 0.2 s
# of a 1 kHz sine at amplitude 0.5; stereo files have the sine in the left
# channel and silence in the right one.
set -e
cd "$(dirname "$0")"
gen() { # <rate> <channels: 1|2> <output> <ffmpeg codec options...>
    rate=$1 ch=$2 out=$3
    shift 3
    if [ "$ch" = 2 ]; then expr='0.5*sin(2*PI*1000*t)|0'; else expr='0.5*sin(2*PI*1000*t)'; fi
    ffmpeg -hide_banner -loglevel error -y -bitexact \
        -f lavfi -i "aevalsrc=$expr:s=$rate:d=0.2" \
        -map_metadata -1 -fflags +bitexact "$@" "$out"
}
gen 44100 2 sine_44100_stereo_s24.wav -c:a pcm_s24le
gen 48000 1 sine_48000_mono_f32.wav -c:a pcm_f32le
gen 31250 1 sine_31250_mono_s16.wav -c:a pcm_s16le
gen 44100 1 sine_44100_mono_s16.aiff -c:a pcm_s16be
gen 44100 1 sine_44100_mono.flac -c:a flac
gen 44100 1 sine_44100_mono.mp3 -c:a libmp3lame -b:a 128k
gen 44100 2 sine_44100_stereo.ogg -c:a libvorbis -q:a 5
gen 44100 1 sine_44100_mono.m4a -c:a aac -b:a 128k
