{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {

  nativeBuildInputs = [
    pkgs.cargo
    pkgs.cargo-flamegraph
    pkgs.cargo-watch
    pkgs.clippy
    pkgs.rust-analyzer
    pkgs.rustc
    pkgs.rustfmt
  ];
}
