{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/2d627a2a704708673e56346fcb13d25344b8eaf3.tar.gz") {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [ cargo rustc ];
}
