let
  unstable = import (fetchTarball https://nixos.org/channels/nixos-unstable/nixexprs.tar.xz) { };
in
{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
    nativeBuildInputs = with pkgs.buildPackages; [
        unstable.rustup
        unstable.pkg-config
        unstable.alsa-lib
        unstable.libjack2
    ];
    LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
}
