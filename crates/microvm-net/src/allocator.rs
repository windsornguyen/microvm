// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Rotating IP allocator for a subnet.
//! Gateway is at base+1. Allocatable range starts at base+2.

use std::net::Ipv4Addr;

pub struct IpAllocator {
    base: u32,
    mask: u32,
    next: u32,
    allocated: Vec<u32>,
}

impl IpAllocator {
    pub fn new(subnet: Ipv4Addr, prefix_len: u8) -> Self {
        let base = u32::from(subnet);
        let mask = !((1u32 << (32 - prefix_len)) - 1);
        Self {
            base: base & mask,
            mask,
            next: (base & mask) + 2,
            allocated: Vec::new(),
        }
    }

    pub fn gateway(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.base + 1)
    }

    pub fn allocate(&mut self) -> Option<Ipv4Addr> {
        let end = self.base | !self.mask;
        let start = self.next;
        let mut candidate = start;
        loop {
            if !self.allocated.contains(&candidate) {
                self.allocated.push(candidate);
                self.next = if candidate + 1 >= end {
                    self.base + 2
                } else {
                    candidate + 1
                };
                return Some(Ipv4Addr::from(candidate));
            }
            candidate += 1;
            if candidate >= end {
                candidate = self.base + 2;
            }
            if candidate == start {
                return None;
            }
        }
    }

    pub fn release(&mut self, addr: Ipv4Addr) {
        let val = u32::from(addr);
        self.allocated.retain(|&a| a != val);
    }

    pub fn available(&self) -> usize {
        let total = (self.base | !self.mask) - self.base - 2;
        total as usize - self.allocated.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alloc_24() -> IpAllocator {
        IpAllocator::new(Ipv4Addr::new(192, 168, 64, 0), 24)
    }

    // --- allocation invariants ---

    #[test]
    fn invariant_first_allocation_is_base_plus_two() {
        let mut a = alloc_24();
        assert_eq!(a.allocate(), Some(Ipv4Addr::new(192, 168, 64, 2)));
    }

    #[test]
    fn invariant_gateway_is_base_plus_one() {
        let a = alloc_24();
        assert_eq!(a.gateway(), Ipv4Addr::new(192, 168, 64, 1));
    }

    #[test]
    fn invariant_sequential_allocations() {
        let mut a = alloc_24();
        assert_eq!(a.allocate(), Some(Ipv4Addr::new(192, 168, 64, 2)));
        assert_eq!(a.allocate(), Some(Ipv4Addr::new(192, 168, 64, 3)));
        assert_eq!(a.allocate(), Some(Ipv4Addr::new(192, 168, 64, 4)));
    }

    #[test]
    fn invariant_release_then_reallocate() {
        let mut a = alloc_24();
        let ip = a.allocate().unwrap();
        a.release(ip);
        assert_eq!(a.available(), 253);
    }

    #[test]
    fn invariant_exhaustion_returns_none() {
        let mut a = IpAllocator::new(Ipv4Addr::new(10, 0, 0, 0), 30);
        // /30 = 4 addresses: .0 (network), .1 (gateway), .2, .3 (broadcast)
        assert!(a.allocate().is_some()); // .2
        assert!(a.allocate().is_none()); // .3 is broadcast (end)
    }

    // --- design decisions ---

    #[test]
    fn design_release_nonexistent_is_noop() {
        let mut a = alloc_24();
        a.release(Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(a.available(), 253);
    }
}
