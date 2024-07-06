let
  unstable = import (fetchTarball https://nixos.org/channels/nixos-unstable/nixexprs.tar.xz) { };
in
{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
    nativeBuildInputs = with pkgs.buildPackages; [ 
        #unstable.cargo
        #unstable.rustc
        #unstable.llvm
        #unstable.clang
        #unstable.elf2uf2-rs
        unstable.rustup
    ];
    LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
}
