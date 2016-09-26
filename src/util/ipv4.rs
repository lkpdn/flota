use bit_vec::BitVec;
use rustc_serialize::Encodable;
use rustc_serialize::Encoder;
use std::result;
use std::fmt;
use std::mem;

use super::errors::*;

#[derive(Debug, Clone)]
pub struct IPv4 {
    addr: Vec<u8>,
    mask: Vec<u8>,
}
impl fmt::Display for IPv4 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.ip(), self.mask_bit())
    }
}
impl Encodable for IPv4 {
    fn encode<S: Encoder>(&self, s: &mut S) -> result::Result<(), S::Error> {
        s.emit_str(format!("{}", self).as_ref())
    }
}
impl IPv4 {
    pub fn from_cidr_notation(s: &str) -> Result<IPv4> {
        let parts: Vec<&str> = s.split("/").collect();
        if parts.len() != 2 {
            return Err("invalid argument".into());
        }
        let decimals: Vec<u8> = parts[0]
            .split(".")
            .map(|p| p.parse::<u8>().unwrap())
            .collect();
        if decimals.len() != 4 {
            return Err("invalid ip part".into());
        }
        let mask_bit = match parts[1].parse::<u8>() {
            Ok(v) if v > 0 && v <= 32 => v,
            Ok(_) | Err(_) => return Err("invalid mask bit".into()),
        };
        let mut mask_bit_vec = BitVec::from_elem(32, false);
        for i in 0..32 {
            if i < mask_bit {
                mask_bit_vec.set(i as usize, true);
            }
        }
        Ok(IPv4 {
            addr: decimals,
            mask: mask_bit_vec.to_bytes(),
        })
    }
    pub fn ip(&self) -> String {
        self.addr
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
            .join(".")
    }
    pub fn mask(&self) -> String {
        self.mask
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
            .join(".")
    }
    pub fn mask_bit(&self) -> u8 {
        let mask = BitVec::from_bytes(self.mask.as_slice());
        mask.iter().filter(|x| *x).count() as u8
    }
    pub fn nw_addr(&self) -> IPv4 {
        let mut nw_bit_vec = BitVec::from_bytes(self.addr.as_slice());
        nw_bit_vec.intersect(&BitVec::from_bytes(self.mask.as_slice()));
        IPv4 {
            addr: nw_bit_vec.to_bytes(),
            mask: self.mask.clone(),
        }
    }
    pub fn nth_sibling(&self, n: i32) -> IPv4 {
        let mut nw_bit_vec = BitVec::from_bytes(self.addr.as_slice());
        let mut narr = unsafe { mem::transmute::<i32, [u8; 4]>(n) };
        narr.reverse();
        let mut narr_bv = BitVec::from_bytes(&narr);
        let mut mask = BitVec::from_bytes(self.mask.as_slice());
        nw_bit_vec.intersect(&mask);
        mask.negate();
        narr_bv.intersect(&mask);
        nw_bit_vec.union(&narr_bv);
        IPv4 {
            addr: nw_bit_vec.to_bytes(),
            mask: self.mask.clone(),
        }
    }
    pub fn incr_node_id(&mut self) -> Result<()> {
        let mut limit = BitVec::from_bytes(self.addr.as_slice());
        let mut mask = BitVec::from_bytes(self.mask.as_slice());
        mask.negate();
        if !limit.union(&mask) {
            return Err("already highest possible node id".into());
        }
        for i in (0..4).rev() {
            if self.mask[i] == 0 || self.addr[i] < self.mask[i] - 1 {
                self.addr[i] += 1;
                return Ok(());
            } else {
                self.addr[i] = 0;
            }
        }
        Ok(())
    }
    pub fn decr_node_id(&mut self) -> Result<()> {
        let mut limit = BitVec::from_bytes(self.addr.as_slice());
        let mask = BitVec::from_bytes(self.mask.as_slice());
        if !limit.intersect(&mask) {
            return Err("already lowest possible node id".into());
        }
        for i in (0..4).rev() {
            if self.addr[i] > 0 {
                self.addr[i] -= 1;
                return Ok(());
            } else {
                self.addr[i] = 0b11111111;
            }
        }
        Ok(())
    }
    pub fn largest_sibling(&self) -> IPv4 {
        let mut sibling = BitVec::from_bytes(self.addr.as_slice());
        let mut mask_bv = BitVec::from_bytes(self.mask.as_slice());
        mask_bv.negate();
        sibling.union(&mask_bv);
        IPv4 {
            addr: sibling.to_bytes(),
            mask: self.mask.clone(),
        }
    }
    pub fn hyphenated(&self) -> String {
        format!("{}-{}",
                self.addr
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>()
                    .join("-"),
                self.mask_bit())
    }
}

#[cfg(test)]
mod tests {
    use super::IPv4;

    #[test]
    fn test_ipv4_new() {
        match IPv4::from_cidr_notation("192.168.1.10/24") {
            Ok(v) => {
                assert_eq!(v.ip(), "192.168.1.10");
                assert_eq!(v.mask(), "255.255.255.0");
                assert_eq!(v.mask_bit(), 24);
            }
            Err(_) => {
                panic!("ipv4 initialisation failed.");
            }
        }
        match IPv4::from_cidr_notation("255.100.255.100/13") {
            Ok(v) => {
                assert_eq!(v.ip(), "255.100.255.100");
                assert_eq!(v.mask(), "255.248.0.0");
                assert_eq!(v.mask_bit(), 13);
            }
            Err(_) => {
                panic!("ipv4 initialisation failed.");
            }
        }
    }
    #[test]
    fn test_ipv4_siblings() {
        let mut ip1 = IPv4::from_cidr_notation("172.16.12.1/24").unwrap();
        ip1.incr_node_id().expect("incr_node_id failed");
        assert_eq!(ip1.ip(), "172.16.12.2");
        ip1.decr_node_id().expect("decr_node_id failed");
        assert_eq!(ip1.ip(), "172.16.12.1");
        ip1.decr_node_id().expect("decr_node_id failed");
        assert_eq!(ip1.ip(), "172.16.12.0");
        match ip1.decr_node_id() {
            Ok(_) => panic!("should be err"),
            Err(_) => {
                assert_eq!(ip1.ip(), "172.16.12.0");
            }
        }
        let mut ip2 = ip1.largest_sibling();
        assert_eq!(ip2.ip(), "172.16.12.255");
        match ip2.incr_node_id() {
            Ok(_) => panic!("should be err"),
            Err(_) => {
                assert_eq!(ip2.ip(), "172.16.12.255");
            }
        }
        assert_eq!(ip2.nth_sibling(77).ip(), "172.16.12.77");
        assert_eq!(ip2.nth_sibling(256).ip(), "172.16.12.0");
        assert_eq!(ip2.nth_sibling(-1).ip(), "172.16.12.255");
    }
}
