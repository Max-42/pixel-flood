use image::RgbImage;
use image::{DynamicImage, GenericImageView};
use indicatif::ProgressBar;
use rand::{Rng, distributions::Alphanumeric};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::net::UdpSocket;
use std::time::Duration;
use std::{env, net::Ipv4Addr};

use ::max_image_sender::{solve_pow, solve_pow_parallel};

fn send_pixel(socket: &UdpSocket, x: u16, y: u16, r: u8, g: u8, b: u8) {
    let mut buf = [0u8; 128];

    buf[0..2].copy_from_slice(&x.to_le_bytes());
    buf[2..4].copy_from_slice(&y.to_le_bytes());

    match socket.send(&buf[0..4]) {
        Ok(_) => {}
        Err(e) => {
            println!("An error occured: {e}");
            return;
        }
    }

    const REQUIRED_BYTES: usize = 8;
    let size = match socket.recv(&mut buf) {
        Ok(size @ REQUIRED_BYTES..usize::MAX) => size,
        Ok(size) => {
            println!("not enough bytes, got {size}, required {REQUIRED_BYTES}");
            return;
        }
        Err(e) => {
            println!("An error occured: {e}");
            return;
        }
    };

    let resp = &buf[..size];
    let difficulty = resp[7];
    let nonce = solve_pow_parallel(resp, difficulty);
    let mut msg = Vec::from(resp);
    msg.extend_from_slice(&nonce);
    msg.push(r);
    msg.push(g);
    msg.push(b);
    socket.send(&msg);
}

/// Size of a pixel request, consits of two u16 = 4 byte
const BYTE_PIXEL_REQUEST: usize = 4;

/// Size of a challange from the server
const BYTE_CHALLENGE: usize = 24;

/// Size of a challenge response to the server
const BYTE_CHALLENGE_RESPONSE: usize = 43;

/// Size of the nonce
const BYTE_NONCE: usize = 16;

/// Ask server for the challenge for given coordinates
fn send_request(socket: &UdpSocket, x: u32, y: u32) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; BYTE_PIXEL_REQUEST];

    let (x, y) = (x as u16, y as u16);

    buf[0..2].copy_from_slice(&x.to_le_bytes());
    buf[2..4].copy_from_slice(&y.to_le_bytes());

    let size = socket.send(&buf)?;
    assert_eq!(size, BYTE_PIXEL_REQUEST, "TODO make this a proper error");
    Ok(())
}

/// Respond to a server challenge
fn solve_challenge(
    socket: &UdpSocket,
    image: &RgbImage,
    challenge: &mut [u8],
) -> Result<(), Box<dyn Error>> {
    if challenge.len() < BYTE_CHALLENGE {
        return Err("challenge too short".into());
    }

    let x = u16::from_le_bytes(challenge[0..2].try_into().unwrap());
    let y = u16::from_le_bytes(challenge[2..4].try_into().unwrap());

    let pixel = image.get_pixel(x.into(), y.into());
    let [r, g, b] = pixel.0;

    let difficulty = challenge[7];

    let nonce = solve_pow_parallel(&challenge[0..BYTE_CHALLENGE], difficulty);
    challenge[BYTE_CHALLENGE..BYTE_CHALLENGE + BYTE_NONCE].copy_from_slice(&nonce);

    challenge[BYTE_CHALLENGE + BYTE_NONCE..BYTE_CHALLENGE_RESPONSE].copy_from_slice(&[r, g, b]);

    let size = socket.send(&challenge)?;
    assert_eq!(
        size, BYTE_CHALLENGE_RESPONSE,
        "TODO make this a proper error"
    );
    Ok(())
}

fn send_image(path: &str) {
    // open image,  convert to rgb8, get its dimensions
    let img = image::open(path).expect("Failed to open image");
    let rgb = img.to_rgb8();
    let (width, height) = img.dimensions();

    // bind to network socket
    let server = "172.29.165.125:8080";
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("Failed to bind socket");
    socket.connect(server);

    socket.set_read_timeout(Some(Duration::from_millis(200)));

    // send the image
    let mut buf = [0u8; BYTE_CHALLENGE_RESPONSE];
    let bar_max = height as u64 * width as u64;
    let bar = ProgressBar::new(bar_max);

    let mut pending_requests = HashSet::new();

    for y in 0..height {
        for x in 0..width {
            // is there an response to be made? If yes, respond!
            match socket.recv(&mut buf) {
                Ok(size) => {
                    solve_challenge(&socket, &rgb, &mut buf).unwrap();
                    bar.inc(1);
                }
                Err(e) => {
                    println!("an error occured: {e}")
                }
            }

            pending_requests.insert((x, y));
            send_request(&socket, x, y).unwrap();
        }
    }

    while bar.position() < bar_max {
        // is there an response to be made? If yes, respond!
        match socket.recv(&mut buf) {
            Ok(size) => {
                solve_challenge(&socket, &rgb, &mut buf).unwrap();
                bar.inc(1);
            }
            Err(_) => todo!(),
        }
    }

    bar.finish();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        return;
    }

    send_image(&args[1]);
}
