use std::{mem, slice};

#[repr(C)]
#[derive(Debug)]
pub struct Region {
    pub offset:   u32,
    pub capacity: u32,
    pub length:   u32,
}

impl Region {
    pub const SIZE: usize = mem::size_of::<Region>();

    pub fn as_bytes(&self) -> &[u8] {
        let ptr = self as *const Self;
        unsafe { slice::from_raw_parts(ptr as *const u8, Self::SIZE) }
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_raw(buf: &[u8]) -> &Self {
        assert_size(buf);
        &*(buf.as_ptr() as *const Self)
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_raw_mut(buf: &mut [u8]) -> &mut Self {
        assert_size(buf);
        &mut *(buf.as_mut_ptr() as *mut Region)
    }
}

fn assert_size(buf: &[u8]) {
    let len = buf.len();
    assert_eq!(len, Region::SIZE, "Incorrect byte size: expecting {}, got {}", Region::SIZE, len);
}
