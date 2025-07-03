use image::GenericImageView;
use image::RgbImage;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::error::Error;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::{env, net::Ipv4Addr};

use ::max_image_sender::solve_pow_parallel;

const UNLIKELY_UDP_ERROR: &str = "unlikely error: UDP socket's send() reports fewer bytes to be sent than in the input datagram. This should never happen for input datagrams below 64KiB";

/// Size of a pixel request, consits of two u16 = 4 byte
const BYTE_PIXEL_REQUEST: usize = 4;

/// Size of a challange from the server
const BYTE_CHALLENGE: usize = 24;

/// Size of a challenge response to the server
const BYTE_CHALLENGE_RESPONSE: usize = 43;

/// Size of the nonce
const BYTE_NONCE: usize = 16;

/// Ask server for the challenge for given coordinates
fn send_request(socket: &UdpSocket, x: u32, y: u32) -> Result<(), std::io::Error> {
    let mut buf = [0u8; BYTE_PIXEL_REQUEST];

    let (x, y) = (x as u16, y as u16);

    buf[0..2].copy_from_slice(&x.to_le_bytes());
    buf[2..4].copy_from_slice(&y.to_le_bytes());

    let size = socket.send(&buf)?;
    assert_eq!(size, BYTE_PIXEL_REQUEST, "{}", UNLIKELY_UDP_ERROR);
    Ok(())
}

#[derive(thiserror::Error, Debug)]
enum ChallengeError {
    #[error("the challenge is not valid, likely a length missmatch?")]
    InvalidChallenge,

    #[error("network error: {0}")]
    NetworkError(#[from] std::io::Error),
}

/// Respond to a server challenge
fn solve_challenge(
    socket: &UdpSocket,
    image: &RgbImage,
    challenge: &mut [u8],
) -> Result<Option<(u32, u32)>, ChallengeError> {
    if challenge.len() < BYTE_CHALLENGE {
        return Err(ChallengeError::InvalidChallenge);
    }

    let x = u16::from_le_bytes(challenge[0..2].try_into().unwrap());
    let y = u16::from_le_bytes(challenge[2..4].try_into().unwrap());

    let pixel = image.get_pixel(x.into(), y.into());

    // fast path, if the pixel already is right, do nothing
    if challenge[4..7] == pixel.0 {
        return Ok(Some((x.into(), y.into())));
    };

    let [r, g, b] = pixel.0;

    let difficulty = challenge[7];

    let nonce = solve_pow_parallel(&challenge[0..BYTE_CHALLENGE], difficulty);
    challenge[BYTE_CHALLENGE..BYTE_CHALLENGE + BYTE_NONCE].copy_from_slice(&nonce);
    challenge[BYTE_CHALLENGE + BYTE_NONCE..BYTE_CHALLENGE_RESPONSE].copy_from_slice(&[r, g, b]);

    let size = socket.send(challenge)?;

    assert_eq!(size, BYTE_CHALLENGE_RESPONSE, "{}", UNLIKELY_UDP_ERROR);

    Ok(None)
}

fn send_image(path: &str) -> Result<(), Box<dyn Error>> {
    // open image,  convert to rgb8, get its dimensions
    let img = image::open(path).expect("Failed to open image");
    let rgb = img.to_rgb8();
    let (width, height) = img.dimensions();

    // bind to network socket
    let server = "172.29.165.125:8080";
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("Failed to bind socket");
    socket.connect(server)?;

    socket.set_nonblocking(true)?;

    // send the image
    let mut buf = [0u8; BYTE_CHALLENGE_RESPONSE];
    let total_pixels = height as u64 * width as u64;

    let mp = MultiProgress::new();

    let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40} {pos:>7}/{len:7} {msg}")
        .unwrap()
        .progress_chars("##-");

    let bar_pixels_done = mp.add(
        ProgressBar::new(total_pixels)
            .with_message("Pixels done")
            .with_style(style.clone()),
    );

    let bar_packets_sent = mp.add(
        ProgressBar::new(0)
            .with_message("Packets sent")
            .with_style(style.clone()),
    );

    let mut finished_pixels = HashSet::new();

    let backoff = std::time::Duration::from_millis(0);

    while finished_pixels.len() as u64 != total_pixels {
        for y in 0..height {
            'pixel_loop: for x in 0..width {
                // if this pixel is known good, skip it
                if finished_pixels.contains(&(x, y)) {
                    continue;
                }

                // is there an response to be made? If yes, respond!
                'udp_receive_loop: loop {
                    match socket.recv(&mut buf) {
                        Ok(_) => {
                            match solve_challenge(&socket, &rgb, &mut buf) {
                                // pixel already has the correct value
                                Ok(Some(finished_pixel_coordinates)) => {
                                    finished_pixels.insert(finished_pixel_coordinates);
                                    bar_pixels_done.inc(1);
                                    continue 'pixel_loop;
                                }

                                // we sent a valid challenge solution, but we don't know if it arrives
                                Ok(None) => {
                                    bar_packets_sent.inc(1);
                                    continue 'pixel_loop;
                                }

                                // if we saturate the net, lets backoff a bit
                                Err(ChallengeError::NetworkError(e))
                                    if e.kind() == ErrorKind::WouldBlock =>
                                {
                                    std::thread::sleep(backoff);
                                }

                                // we failed to sent a challenge solution
                                Err(e) => {
                                    bar_packets_sent.inc(1);
                                    println!("an error occured sending a solution: {e:?}");
                                }
                            }
                        }

                        // we don't care if this recv would block
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break 'udp_receive_loop,

                        // we do care for other errors
                        Err(e) => {
                            println!("an error occured receiving: {e:?}")
                        }
                    }
                }

                match send_request(&socket, x, y) {
                    Ok(_) => {}

                    // we don't care if this recv would block
                    Err(e) if e.kind() == ErrorKind::WouldBlock => continue,

                    // we don't care if we saturate the network
                    Err(e) if e.kind() == ErrorKind::ResourceBusy => {
                        std::thread::sleep(backoff);
                        continue;
                    }

                    // we do care for other errors
                    Err(e) => {
                        println!("an error occured sending request: {e}")
                    }
                }
                bar_packets_sent.inc(1);
            }
        }
    }

    bar_pixels_done.finish();

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        return;
    }

    send_image(&args[1]).unwrap();
}
