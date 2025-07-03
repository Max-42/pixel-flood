use image::GenericImageView;
use rand::{distributions::Alphanumeric, Rng};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::net::UdpSocket;
use std::time::Duration;

use ::max_image_sender::{solve_pow, solve_pow_parallel};

fn send_pixel(x: u16, y: u16, r: u8, g: u8, b: u8) {
    let server = "172.29.165.125:8080";
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        let mut message = vec![];
        message.extend_from_slice(&x.to_le_bytes());
        message.extend_from_slice(&y.to_le_bytes());

        let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));

        if socket.send_to(&message, server).is_ok() {
            let mut buf = [0u8; 1024];
            if let Ok((size, _)) = socket.recv_from(&mut buf) {
                let resp = &buf[..size];
                if resp.len() >= 8 {
                    let difficulty = resp[7];
                    let nonce = solve_pow_parallel(resp, difficulty);
                    let mut msg = Vec::from(resp);
                    msg.extend_from_slice(&nonce);
                    msg.push(r);
                    msg.push(g);
                    msg.push(b);
                    let _ = socket.send_to(&msg, server);
                }
            }
        }
    }
}

fn send_image(path: &str) {
    let img = image::open(path).expect("Failed to open image");
    let rgb = img.to_rgb8();
    let (width, height) = img.dimensions();

    for y in 0..height {
        for x in 0..width {
            let pixel = rgb.get_pixel(x, y);
            let [r, g, b] = pixel.0;
            send_pixel(x as u16, y as u16, r, g, b);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        return;
    }

    send_image(&args[1]);
}
