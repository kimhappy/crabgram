{
  runCommand,
  gcc15,
  zstd,
  inputs,
}:
let
  zstdGcc = gcc15.cc.overrideAttrs (old: {
    buildInputs = old.buildInputs ++ [ zstd ];
    configureFlags = old.configureFlags ++ [
      "--with-zstd-include=${zstd.dev}/include"
      "--with-zstd-lib=${zstd.out}/lib"
    ];
  });

  prefix = gcc15.targetPrefix;
in
runCommand "libnickel"
  {
    nativeBuildInputs = [
      zstdGcc
      gcc15.bintools.bintools
    ];
  }
  ''
    ${prefix}gcc -mcpu=cortex-a76.cortex-a55 -fPIC -ffunction-sections -fdata-sections \
        -r -nostdlib -flinker-output=nolto-rel -Wl,--gc-sections \
        -Wl,-u,nickel_init,-u,nickel_license_run_begin,-u,nickel_license_run_silence,-u,nickel_license_interface \
        -Wl,--whole-archive ${inputs.plugin-builder}/plugins-dep/package/libnickel/libnickel-gcc15.a \
        -o nickel.o
    ${prefix}objcopy --strip-symbol=_unused_exposed_symbol_function nickel.o

    mkdir -p $out/lib
    ${prefix}ar rcs $out/lib/libnickel.a nickel.o
  ''
