{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {

  nativeBuildInputs = [
    pkgs.cargo
    pkgs.cargo-flamegraph
    pkgs.cargo-watch
    pkgs.rust-analyzer
    pkgs.rustc
    pkgs.rustfmt
  ];
}
