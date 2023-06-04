#!/bin/bash
set -euo pipefail

# this script generates `bindings_wrapper.h`, which should include
# all the C functions/structs for which we would like `bindgen`
# to create Rust bindings. For now, we want to be able to call
# any core Nautilus function in the headers in `include/nautilus`.

OUTF=bindgen_wrapper.h

echo -n > $OUTF
echo '// TODO: dont hardcode these. They should be parameterized'       >> $OUTF
echo '// according to the options set in Kbuild'                        >> $OUTF
echo '#define NAUT_CONFIG_MAX_CPUS 1'                                   >> $OUTF
echo '#define NAUT_CONFIG_MAX_THREADS 1'                                >> $OUTF
echo '#define NAUT_CONFIG_MAX_IOAPICS 1'                                >> $OUTF
echo '#define NAUT_CONFIG_X86_64_HOST'                                  >> $OUTF
echo '#define __NAUTILUS__'                                             >> $OUTF
echo ''                                                                 >> $OUTF
echo '// TODO: perhaps rename the `main` function in'                   >> $OUTF
echo '// `include/arch/x64/main.h` because (annoyingly)'                >> $OUTF
echo '// clang treats a wrongly-typed `main` as a hard error'           >> $OUTF
echo '// (ie. forcing the signature to be `int main(int x, char** y)`)' >> $OUTF
echo '#define BINDGEN_ELIDE_MAIN'                                       >> $OUTF
echo ''                                                                 >> $OUTF

for f in $(find ../../include/config/debug -name "*.h" -exec realpath --relative-to ../../include {} \;); do
  echo "#include \"$f\"" >> $OUTF
done

for f in $(find ../../include/config/rust -name "*.h" -exec realpath --relative-to ../../include {} \;); do
  echo "#include \"$f\"" >> $OUTF
done

for f in ../../include/nautilus/*.h; do
  name=$(basename $f)
  if [ "$name" = "linker.h" ]; then
      echo '// TODO: issue with redefinition of `struct symentry`' >> $OUTF
      echo '// #include "nautilus/linker.h"'                       >> $OUTF
  elif [ "$name" = "realmode.h" ]; then
      echo '// TODO: issue with packed+aligned struct `nk_real_mode_int_args`' >> $OUTF
      echo '// #include "nautilus/realmode.h"'                                 >> $OUTF
  else
      echo "#include \"nautilus/$name\"" >> $OUTF
  fi
done

echo "#include \"dev/virtio_pci.h\"" >> $OUTF
