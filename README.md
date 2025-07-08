This is a Client for

https://gitli.stratum0.org/led-disp/pixel-chain / https://github.com/P1x31Cha1n/P1x31Cha1n




RUSTFLAGS="-C target-cpu=native" cargo run --release smile-png.png


while true; do python clock.py && RUSTFLAGS="-C target-cpu=native" cargo run --release transparent_clock.png; sleep 0.3; done
