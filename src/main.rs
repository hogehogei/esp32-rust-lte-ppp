#[path = "./serial_port.rs"]
mod serial_port;
use crate::serial_port::SerialPort;
use std::io::{Read, Write};

use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::uart::*;

use anyhow::{anyhow, Context};

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
            continue;
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
            _ => {}
        }
    }

    Ok(response)
}

fn modem_wait_response(serial_port: &mut SerialPort) -> Result<String, anyhow::Error>
{

    loop {
        let response = modem_wait_readline(serial_port)?;
        log::info!("Modem: [{}]", response);
        
        if response == "OK" {
            return Ok(response)
        }
        if response.contains("ERROR") {
            return Err(anyhow!("AT command error response : {}", response));
        }
    }
}

fn init_lte_modem(serial_port: &mut SerialPort) -> Result<(), anyhow::Error>
{
    log::info!("init_lte_modem.");
    
    const CSQ : &str = "AT+CSQ\r";
    serial_port.write(CSQ.as_bytes()).with_context( || "serial_port write error" )?;
    modem_wait_response(serial_port)?;

    const ATE0 : &str = "ATE0\r";
    serial_port.write(ATE0.as_bytes()).with_context( || "serial_port write error" )?;
    modem_wait_response(serial_port)?;

    const CGDCONT : &str = "AT+CGDCONT=1,\"IP\",\"povo.jp\"\r";
    serial_port.write(CGDCONT.as_bytes()).with_context( || "serial_port write error" )?;
    modem_wait_response(serial_port)?;

    const ATD : &str = "ATD*99##\r";
    serial_port.write(ATD.as_bytes()).with_context( || "serial_port write error" )?;
    modem_wait_response(serial_port)?;

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
