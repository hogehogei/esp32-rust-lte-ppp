#[path = "./serial_port.rs"]
mod serial_port;
use crate::serial_port::SerialPort;
use std::io::{Read, Write};

use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::uart::*;

use anyhow::{anyhow, Context};

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
            Err(e) => { err_n += 1; log::warn!("init_lte_modem Error: {}, retry cmd={}", e, cmd); }
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

    loop {}
    Ok(())
}
