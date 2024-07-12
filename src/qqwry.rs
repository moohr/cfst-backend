/*
    * QQWry IP database parser
    * Special thanks to @lilydjwg for the original code
*/
use std::fs::File;
use std::net::Ipv4Addr;
use std::path::Path;

use byteorder::{ByteOrder, LittleEndian};
use eyre::Result;
use memmap2::Mmap;

pub struct QQWry {
    mmap: Mmap,
    offset: usize,
    count: u32,
}

#[derive(Debug)]
pub struct IpInfo {
    pub start_ip: Ipv4Addr,
    pub end_ip: Ipv4Addr,
    pub country: String,
    pub area: String,
}

impl QQWry {
    pub fn new() -> Result<Self> {
        let file = File::open("assets/qqwry.dat")?;
        let mmap = unsafe { Mmap::map(&file)? };
        let first = LittleEndian::read_u32(&mmap[0..4]);
        let last = LittleEndian::read_u32(&mmap[4..8]);
        Ok(Self {
            mmap,
            offset: first as usize,
            count: (last - first) / 7,
        })
    }

    pub fn lookup(&self, ip: Ipv4Addr) -> Option<IpInfo> {
        let mut si = 0;
        let mut ei = self.count;
        if ip < self.read_index(si).0 {
            return None;
        }
        if ip >= self.read_index(ei).0 {
            si = ei;
        } else {
            // keep si <= ip < ei
            while (si + 1) < ei {
                let mi = (si + ei) / 2;
                if self.read_index(mi).0 <= ip {
                    si = mi;
                } else {
                    ei = mi;
                }
            }
        }
        let ipinfo = self.read_info(si);
        if ip > ipinfo.end_ip {
            None
        } else {
            Some(ipinfo)
        }
    }

    fn read_index(&self, n: u32) -> (Ipv4Addr, usize) {
        let pos = self.offset + 7 * n as usize;
        let mut data: [u8; 8] = [0; 8];
        data[..7].copy_from_slice(&self.mmap[pos..pos + 7]);
        let ip = LittleEndian::read_u32(&data[0..4]).into();
        let offset = LittleEndian::read_u32(&data[4..8]) as usize;
        (ip, offset)
    }

    fn read_info(&self, index: u32) -> IpInfo {
        let index = self.read_index(index);
        let sip = index.0;
        let pos = index.1;
        let eip = LittleEndian::read_u32(&self.mmap[pos..pos + 4]).into();
        let (country, mut area) = self.read_record(pos + 4);
        if area == " CZ88.NET" {
            area = String::new();
        }
        IpInfo {
            start_ip: sip,
            end_ip: eip,
            country,
            area,
        }
    }

    fn read_record(&self, pos: usize) -> (String, String) {
        let mode = self.mmap[pos];
        let country;
        let area;
        match mode {
            0x01 => {
                let rp = self.read_3byte_offset(pos);
                (country, area) = self.read_record(rp);
            }
            0x02 => {
                let rp = self.read_3byte_offset(pos);
                country = self.read_country(rp);
                area = self.read_area(pos + 4);
            }
            _ => {
                let (c, len) = self.read_string(pos);
                country = c;
                area = self.read_area(pos + len);
            }
        }

        (country, area)
    }

    fn read_country(&self, pos: usize) -> String {
        let mode = self.mmap[pos];
        match mode {
            0x02 => {
                let rp = self.read_3byte_offset(pos);
                self.read_country(rp)
            }
            _ => self.read_string(pos).0,
        }
    }

    fn read_area(&self, pos: usize) -> String {
        let mode = self.mmap[pos];
        match mode {
            0x01 | 0x02 => {
                let rp = self.read_3byte_offset(pos);
                self.read_area(rp)
            }
            _ => self.read_string(pos).0,
        }
    }

    fn read_3byte_offset(&self, pos: usize) -> usize {
        let x = LittleEndian::read_u32(&self.mmap[pos..pos + 4]);
        (x >> 8) as usize
    }

    fn read_string(&self, pos: usize) -> (String, usize) {
        let slice = &self.mmap[pos..];
        let bytes = slice.split(|&b| b == b'\0').next().unwrap();
        let s = encoding_rs::GBK.decode_without_bom_handling(bytes).0.into();
        (s, bytes.len() + 1)
    }
}
