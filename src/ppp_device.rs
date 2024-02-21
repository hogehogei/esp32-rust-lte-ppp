use ppproto::pppos::{PPPoS, PPPoSAction};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;
use crate::serial_port::SerialPort;

use std::io::{Write, Read};

const MTU: usize = 1520; // IP mtu of 1500 + some margin for PPP headers.

pub struct PPPDevice {
    pub ppp: PPPoS<'static>,
    port: SerialPort<'static>,
    rx_buf: [u8; MTU],
}

impl PPPDevice {
    pub fn new(ppp: PPPoS<'static>, port: SerialPort<'static>) -> Self {
        Self {
            ppp,
            port,
            rx_buf: [0; MTU],
        }
    }
}

impl Device for PPPDevice {
    type RxToken<'t> = PPPRxToken<'t> where Self: 't;
    type TxToken<'t> = PPPTxToken<'t> where Self: 't;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> 
    {
        log::info!("PPPDevice->receive start.");
        self.port.set_nonblocking(true).unwrap();

        let mut tx_buf = [0; 2048];

        let mut read_buf = [0; 2048];
        let mut data: &[u8] = &[];
        loop {
            // Poll the ppp
            log::info!("PPPDevice pooling start.");
            match self.ppp.poll(&mut tx_buf, &mut self.rx_buf) {
                PPPoSAction::None => {
                    log::info!("PPPoSAction::None");
                }
                PPPoSAction::Transmit(n) => {
                    log::info!("PPPoSAction::Transmit");
                    if let Err(e) = self.port.write_all(&tx_buf[..n]) {
                        log::warn!("error when PPPoSAction::Transmit, reason={}", e);
                        return None;
                    }
                },
                PPPoSAction::Received(range) => {
                    log::info!("PPPoSAction::Received");
                    return Some((
                        PPPRxToken {
                            buf: &mut self.rx_buf[range],
                        },
                        PPPTxToken {
                            port: &mut self.port,
                            ppp: &mut self.ppp,
                        },
                    ));
                }
            }

            // If we have no data, read some.
            if data.len() == 0 {
                let n = match self.port.read(&mut read_buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return None,
                    Err(e) => panic!("error reading: {:?}", e),
                };
                data = &read_buf[..n];
            }

            // Consume some data, saving the rest for later
            let n = self.ppp.consume(data, &mut self.rx_buf);
            data = &data[n..];
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(PPPTxToken {
            port: &mut self.port,
            ppp: &mut self.ppp,
        })
    }

    /// Get a description of device capabilities.
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps: DeviceCapabilities = Default::default();
        caps.max_transmission_unit = 1500;
        caps.medium = Medium::Ip;
        caps
    }
}

pub struct PPPRxToken<'a> {
    buf: &'a mut [u8],
}

impl<'a> RxToken for PPPRxToken<'a> {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.buf)
    }
}

pub struct PPPTxToken<'a> {
    port: &'a mut SerialPort<'static>,
    ppp: &'a mut PPPoS<'static>,
}

impl<'a> TxToken for PPPTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut pkt_buf = [0; 2048];
        let pkt = &mut pkt_buf[..len];
        let r = f(pkt);

        let mut tx_buf = [0; 2048];
        let n = self.ppp.send(pkt, &mut tx_buf).unwrap();

        // not sure if this is necessary
        self.port.set_nonblocking(false).unwrap();

        self.port.write_all(&tx_buf[..n]).unwrap();

        r
    }
}