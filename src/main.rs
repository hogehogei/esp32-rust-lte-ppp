#[path = "./serial_port.rs"]
mod serial_port;
use crate::serial_port::SerialPort;
use std::io::{Read, Write, Error, ErrorKind};
use std::fmt::Write as _;

use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::uart::*;
use anyhow::{anyhow, Context};

#[path = "./ppp_device.rs"]
mod ppp_device;
use crate::ppp_device::PPPDevice;
use ppproto::pppos::PPPoS;

use smoltcp::iface::{Interface, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::IpCidr;

const RETRY_TIME: i32 = 5;

#[derive(PartialEq)]
enum ModelReadSeq
{
    BeginCR,
    BeginLF,
    Data,
    EndLF,
}

fn modem_wait_readline(serial_port: &mut SerialPort) -> Result<String, anyhow::Error>
{
    let mut buf : [u8; 1] = [0];
    let mut response : String = String::from("");
    let mut step = ModelReadSeq::BeginCR;

    loop {
        let n = serial_port.read(&mut buf).with_context(|| "serial_port read error.")?;
        if n == 0 {
            break;
        }

        match step {
            ModelReadSeq::BeginCR => { 
                if buf[0] == b'\r' {
                    step = ModelReadSeq::BeginLF;
                }
            },
            ModelReadSeq::BeginLF => { 
                if buf[0] == b'\n' {
                    step = ModelReadSeq::Data;
                }
            },
            ModelReadSeq::Data => {
                if buf[0] == b'\r' {
                    step = ModelReadSeq::EndLF;
                }
                else {
                    response += std::str::from_utf8(&buf)
                    .with_context(|| "serial_port invalid character.")?;
                }
            },
            ModelReadSeq::EndLF => {
                if buf[0] == b'\n' {
                    break;
                }
            },
        }
    }

    if step != ModelReadSeq::EndLF {
        log::warn!("serial_port read cannot complete: {}", response);
        return Ok(String::from(""))
    }
    
    Ok(response)
}

fn modem_wait_response(serial_port: &mut SerialPort) -> Result<String, anyhow::Error>
{
    loop {
        let response = modem_wait_readline(serial_port)?;
        log::info!("Modem: [{}]", response);
        
        if response == "" {
            break;
        }
        if response == "OK" {
            return Ok(response);
        }
        if response.contains("ERROR") {
            return Err(anyhow!("AT command error response : {}", response));
        }
    }

    Err(anyhow!("No correct response from modem."))
}

fn send_cmd(serial_port: &mut SerialPort, cmd: &str) -> Result<(), anyhow::Error>
{
    serial_port.write(cmd.as_bytes()).with_context( || "serial_port write error" )?;
    modem_wait_response(serial_port)?;

    Ok(())
}

fn send_cmd_retry(serial_port: &mut SerialPort, cmd: &str) -> Result<(), anyhow::Error>
{
    let mut err_n = 0;
    
    for _i in 0..RETRY_TIME {
        match send_cmd(serial_port, cmd) {
            Ok(_s) => { break; },
            Err(e) => { 
                err_n += 1; 
                log::warn!("init_lte_modem Error: {}, retry cmd={}", e, cmd); 
            }
        }
    }
    if err_n >= RETRY_TIME {
        return Err(anyhow!("send_cmd retry failed. cmd={}", cmd));
    }
    Ok(())
}

fn init_lte_modem(serial_port: &mut SerialPort) -> Result<(), anyhow::Error>
{
    log::info!("init_lte_modem.");

    const CSQ : &str = "AT+CSQ\r";
    send_cmd_retry(serial_port, CSQ)?;

    const ATE0 : &str = "ATE0\r";
    send_cmd_retry(serial_port, ATE0)?;

    const CGDCONT : &str = "AT+CGDCONT=1,\"IP\",\"povo.jp\"\r";
    send_cmd_retry(serial_port, CGDCONT)?;
    
    const ATD : &str = "ATD*99##\r";
    send_cmd_retry(serial_port, ATD)?;

    Ok(())
}

fn main() -> anyhow::Result<(), anyhow::Error> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("esp32-rust-lte-ppp sample start.");

    log::info!("Initialize serial port.");
    let peripherals = Peripherals::take()?;
    let config = config::Config::new().baudrate(Hertz(115_200));
    let mut serial_port = SerialPort::new(
        peripherals.pins.gpio17,
        peripherals.pins.gpio18,
        peripherals.uart1,
        &config
    )?;

    init_lte_modem(&mut serial_port)?;

    let ppp_config = ppproto::Config {
        username: b"povo2.0",
        password: b"",
    };
    let mut ppp = PPPoS::new(ppp_config);
    ppp.open().map_err(|_e| Error::new(ErrorKind::Other, "ppp open() failed. InvalidStateError"))?;

    let mut ppp_device = PPPDevice::new(ppp, serial_port);

    let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 64]);
    let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 128]);
    let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

    let mut iface_config = smoltcp::iface::Config::new(smoltcp::wire::HardwareAddress::Ip);
    iface_config.random_seed = rand::random();
    let mut iface = Interface::new(iface_config, &mut ppp_device, Instant::now());
    let mut sockets = SocketSet::new(vec![]);
    let tcp1_handle = sockets.add(tcp_socket);

    loop {
        let timestamp = Instant::now();
        iface.poll(timestamp, &mut ppp_device, &mut sockets);

        let status = ppp_device.ppp.status();

        if let Some(ipv4) = status.ipv4 {
            if let Some(want_addr) = ipv4.address {
                // convert to smoltcp
                let want_addr = smoltcp::wire::Ipv4Address::from_bytes(&want_addr.0);
                iface.update_ip_addrs(|addrs| {
                    if addrs.len() != 1 || addrs[0].address() != want_addr.into() {
                        addrs.clear();
                        addrs.push(IpCidr::new(want_addr.into(), 0)).unwrap();
                        log::info!("Assigned a new IPv4 address: {}", want_addr);
                    }
                });
            }
        }

        // tcp:6969: respond "hello"
        {
            let socket = sockets.get_mut::<tcp::Socket>(tcp1_handle);
            if !socket.is_open() {
                socket.listen(6969).unwrap();
            }

            if socket.can_send() {
                log::info!("tcp:6969 send greeting");
                write!(socket, "hello\n").unwrap();
                log::info!("tcp:6969 close");
                socket.close();
            }
        }
    }

    Ok(())
}
