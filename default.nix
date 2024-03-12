{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/2d627a2a704708673e56346fcb13d25344b8eaf3.tar.gz") {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    cargo
    libiconv
    protobuf
    rustc
    rustfmt
  ] ++ (with darwin.apple_sdk; [
    frameworks.AppKit
    frameworks.Vision
  ]);

  shellHook = ''
    alias vim=nvim

    # lancedb has some nonsense expecting this to be set
    export RUSTUP_TOOLCHAIN=stable
  '';
}

