//! Generic page table interface
//!
//! Implemented for every architecture, used by OS.

use super::*;
#[cfg(test)]
pub use self::mock_page_table::MockPageTable;

#[cfg(test)]
mod mock_page_table;

/// Adaptable page table interface
pub trait PageTable {
    type Entry: Entry;

    /// Maps a page to a frame, returns corresponding page entry.
    fn map(&mut self, addr: VirtAddr, target: PhysAddr) -> &mut Self::Entry;

    /// Unmap a virtual page.
    fn unmap(&mut self, addr: VirtAddr);

    /// Get the page entry of a virtual address.
    fn get_entry(&mut self, addr: VirtAddr) -> &mut Self::Entry;

    /// Get raw content in the page. Only used for testing with mock.
    fn get_page_slice_mut<'a,'b>(&'a mut self, addr: VirtAddr) -> &'b mut [u8];

    /// Read data at a virtual address. Only used for testing with mock.
    fn read(&mut self, addr: VirtAddr) -> u8;

    /// Write data to a virtual address. Only used for testing with mock.
    fn write(&mut self, addr: VirtAddr, data: u8);
}

/// Page entry interface
pub trait Entry {
    /// IMPORTANT!
    /// This must be called after any change to ensure it become effective.
    /// Usually this will make a flush to TLB/MMU.
    fn update(&mut self);

    /// Will be set when accessed
    fn accessed(&self) -> bool;
    /// Will be set when written
    fn dirty(&self) -> bool;
    /// Will PageFault when try to write page where writable=0
    fn writable(&self) -> bool;
    /// Will PageFault when try to access page where present=0
    fn present(&self) -> bool;

    fn clear_accessed(&mut self);
    fn clear_dirty(&mut self);
    fn set_writable(&mut self, value: bool);
    fn set_present(&mut self, value: bool);

    fn target(&self) -> PhysAddr;
    fn set_target(&mut self, target: PhysAddr);

    // For Copy-on-write extension
    fn writable_shared(&self) -> bool;
    fn readonly_shared(&self) -> bool;
    fn set_shared(&mut self, writable: bool);
    fn clear_shared(&mut self);

    // For Swap extension
    fn swapped(&self) -> bool;
    fn set_swapped(&mut self, value: bool);

    fn user(&self) -> bool;
    fn set_user(&mut self, value: bool);
    fn execute(&self) -> bool;
    fn set_execute(&mut self, value: bool);
}
