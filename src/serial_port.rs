use std::io;
use std::io::{Error, ErrorKind};

use esp_idf_svc::hal::peripheral::Peripheral;
use esp_idf_svc::hal::uart::*;
use esp_idf_svc::hal::gpio;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::io::Write;

use crate::dump;

pub struct SerialPort<'a> {
    uart: UartDriver<'a>,
}

static SERIAL_READ_TIMEOUT : u32 = 100;

impl<'a> SerialPort<'a> {
    pub fn new( tx : impl Peripheral<P = impl OutputPin> + 'a,
                rx : impl Peripheral<P = impl InputPin> + 'a,
                uart : impl Peripheral<P = impl Uart> + 'a,
                uart_config: &esp_idf_svc::hal::uart::config::Config) -> io::Result<Self> {
    
        let uart = UartDriver::new(
            uart,
            tx,
            rx,
            Option::<gpio::Gpio0>::None,
            Option::<gpio::Gpio1>::None,
            &uart_config,
        )
        .map_err(|esp_errcode| Error::new(ErrorKind::Other, esp_errcode))?;

        return Ok(Self{uart})
    }

    pub fn set_nonblocking(&self, _nonblocking: bool) -> io::Result<()>
    {
        // ノンブロッキング実装はとりあえずしない
        Ok(())
    }
}

impl<'a> io::Read for SerialPort<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        log::info!("SerialPort::read");
        dump::hexdump(buf);

        self.uart.read(buf, SERIAL_READ_TIMEOUT)
        .map_err(|esp_errcode| Error::new(ErrorKind::Other, esp_errcode))
    }
}

impl<'a> io::Write for SerialPort<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        log::info!("SerialPort::write");
        dump::hexdump(buf);

        self.uart.write(buf)
        .map_err(|esp_errcode| Error::new(ErrorKind::Other, esp_errcode))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.uart.flush()
        .map_err(|esp_errcode| Error::new(ErrorKind::Other, esp_errcode))
    }
}
