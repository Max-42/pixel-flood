use image::GenericImageView;
use image::RgbaImage;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use socket2::SockRef;
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashSet;
use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::{env, thread, time::Duration};

use ::max_image_sender::solve_pow_parallel;

const UNLIKELY_UDP_ERROR: &str = "unlikely error: UDP socket's send() reports fewer bytes to be sent than in the input datagram. This should never happen for input datagrams below 64KiB";
const BYTE_PIXEL_REQUEST: usize = 4;
const BYTE_CHALLENGE: usize = 24;
const BYTE_CHALLENGE_RESPONSE: usize = 43;
const BYTE_NONCE: usize = 16;

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

fn solve_challenge(
    socket: &UdpSocket,
    image: &RgbaImage,
    challenge: &mut [u8],
) -> Result<Option<(u32, u32)>, ChallengeError> {
    if challenge.len() < BYTE_CHALLENGE {
        return Err(ChallengeError::InvalidChallenge);
    }

    let x = u16::from_le_bytes(challenge[0..2].try_into().unwrap());
    let y = u16::from_le_bytes(challenge[2..4].try_into().unwrap());
    let pixel = image.get_pixel(x.into(), y.into());

    let [r, g, b, a] = pixel.0;

    if challenge[4..7] == [r, g, b] {
        return Ok(Some((x.into(), y.into())));
    }

    if a == 0 {
        return Ok(Some((x.into(), y.into())));
    }

    let difficulty = challenge[7];

    let nonce = solve_pow_parallel(&challenge[0..BYTE_CHALLENGE], difficulty);
    challenge[BYTE_CHALLENGE..BYTE_CHALLENGE + BYTE_NONCE].copy_from_slice(&nonce);
    challenge[BYTE_CHALLENGE + BYTE_NONCE..BYTE_CHALLENGE_RESPONSE].copy_from_slice(&[r, g, b]);

    let size = socket.send(challenge)?;
    assert_eq!(size, BYTE_CHALLENGE_RESPONSE, "{}", UNLIKELY_UDP_ERROR);

    Ok(None)
}

fn send_image(path: &str) -> Result<(), Box<dyn Error>> {
    let img = image::open(path).expect("Failed to open image");
    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    let server_addr = "172.29.165.125:8080";
    let server: SocketAddr = server_addr.parse()?;

    // Use socket2 to configure buffer sizes
    let socket2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket2.set_send_buffer_size(64 * 1024 * 1024)?;
    socket2.set_recv_buffer_size(64 * 1024 * 1024)?;
    socket2.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)).into())?;

    // Convert back to std::net::UdpSocket
    let socket: UdpSocket = socket2.into();
    socket.connect(server)?;
    socket.set_nonblocking(true)?;

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

    let backoff = Duration::from_millis(1);

    let mut pixels_missing: HashSet<(u32, u32)> = (0..width )
        .into_iter()
        .flat_map(|x| (0..height ).into_iter().map(move |y| (x.clone(), y)))
        .collect();

    while  !pixels_missing.is_empty() {
        let mut finished_pixels = HashSet::new();
        'pixel_loop: for (x, y) in pixels_missing.iter().cloned() {
            if finished_pixels.contains(&(x, y)) {
                continue;
            }

            if x >= 384 || y >= 256 {
                finished_pixels.insert((x, y));
                bar_pixels_done.inc(1);
                continue;
            }

            if rgba.get_pixel(x, y)[3] == 0 {
                finished_pixels.insert((x, y));
                bar_pixels_done.inc(1);
                continue;
            }

            'udp_receive_loop: loop {
                match socket.recv(&mut buf) {
                    Ok(_) => match solve_challenge(&socket, &rgba, &mut buf) {
                        Ok(Some(coords)) => {
                            finished_pixels.insert(coords);
                            bar_pixels_done.inc(1);
                            continue 'pixel_loop;
                        }
                        Ok(None) => {
                            bar_packets_sent.inc(1);
                            continue 'pixel_loop;
                        }
                        Err(ChallengeError::NetworkError(e))
                            if e.kind() == ErrorKind::WouldBlock =>
                        {
                            thread::sleep(backoff);
                        }
                        Err(e) => {
                            bar_packets_sent.inc(1);
                            println!("Error sending solution: {e:?}");
                        }
                    },
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break 'udp_receive_loop,
                    Err(e) => println!("Recv error: {e:?}"),
                }
            }

            match send_request(&socket, x, y) {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                Err(e) if e.kind() == ErrorKind::ResourceBusy => {
                    thread::sleep(backoff);
                    continue;
                }
                Err(e) => println!("Error sending request: {e}"),
            }

            bar_packets_sent.inc(1);
        }

        pixels_missing = pixels_missing.difference(&finished_pixels).map(|(x, y)| (*x, *y)).collect();
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
