#!/bin/bash
set -euo pipefail

if objdump -d libnk_rust.a | grep -q 'call.*<fmod'; then
    echo "fmod_hack.sh: 'fmod' is not dead code! Aborting." >&2
    exit 1
else
    objcopy --strip-symbol fmod libnk_rust.a
fi
