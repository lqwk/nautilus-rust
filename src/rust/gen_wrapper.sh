#!/bin/bash
set -euo pipefail

# this script generates `bindings_wrapper.h`, which should include
# all the C functions/structs for which we would like `bindgen`
# to create Rust bindings. For now, we want to be able to call
# any core Nautilus function in the headers in `include/nautilus`.

OUTF=bindgen_wrapper.h

echo -n > $OUTF
for f in $(find ../../include/config/debug -name "*.h" -exec realpath --relative-to ../../include {} \;); do
  echo "#include \"$f\"" >> $OUTF
done

for f in ../../include/nautilus/*.h; do
  name=$(basename $f)
  echo "#include \"nautilus/$name\"" >> $OUTF
done

