#!/bin/sh
# Copies the licences shipped with a build into directory $1: the program's
# own (GPL-3.0-or-later), the SYRO library's (BSD-3-Clause) and the fonts'
# (SIL OFL 1.1). Run from the repository root.
set -eu
mkdir -p "$1"
cp LICENSE "$1/LICENSE"
cp volcasm-syro-sys/vendor/volcasample/COPYING "$1/SYRO-COPYING"
cp volcasm-app/assets/fonts/OFL-*.txt "$1/"
