{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/2d627a2a704708673e56346fcb13d25344b8eaf3.tar.gz") {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    cargo
    lldb
    libiconv
    rustc
    rustfmt
  ] ++ (with darwin.apple_sdk; [
    frameworks.AppKit
  ]);

  shellHook = ''
    alias vim=nvim
    export PS1="\[\033[1;32m\][esc:\w]\$\[\033[0m\] "
  '';
}

