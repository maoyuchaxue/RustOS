//! Memory set implementation
//! 
//! MemorySet<T> represents all memory-related info needed by a thread.
//! 
//! Including:
//! 
//! + areas: Vec<MemoryArea>, valid memory areas.
//! + page_table: T, inactive page table, will be activated during context switch.
//! + kstack: Stack, start/end point of kernel stack.
//! 
//! A detailed description may be found in [rust-os-docs](https://rucore.gitbook.io/rust-os-docs/nei-cun-guan-li-mo-kuai) (in Chinese).

use alloc::vec::Vec;
use core::fmt::{Debug, Error, Formatter};
use super::*;
use paging::*;

/// An inactive, temporarily uneditable page table
pub trait InactivePageTable {
    /// Associated type: active, editable page table
    type Active: PageTable;

    /// Creates a new page table, sets recursive mapping and maps kernel space.
    fn new() -> Self;

    /// Creates a new page table but sets recursive mapping only.
    fn new_bare() -> Self;

    /// Edits the page table content with a function f.
    fn edit(&mut self, f: impl FnOnce(&mut Self::Active));

    unsafe fn activate(&self);

    /// Activates the page table temporarily and apply function f.
    unsafe fn with(&self, f: impl FnOnce());

    /// Returns CR3(x86_64)/satp(RISC-V) when the page table is valid.
    fn token(&self) -> usize;

    /// Alloc a physical frame for page table entry storage. Used by MemoryArea.
    fn alloc_frame() -> Option<PhysAddr>;

    /// Dealloc a physical frame. Used by MemoryArea.
    fn dealloc_frame(target: PhysAddr);

    /// Alloc kernel stack. Used at MemorySet initialization.
    fn alloc_stack() -> Stack;
}

/// 一片连续内存空间，有相同的访问权限
/// 对应ucore中 `vma_struct`
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct MemoryArea {
    start_addr: VirtAddr,
    end_addr: VirtAddr,
    phys_start_addr: Option<PhysAddr>, // can either be mapped or not
    flags: MemoryAttr,
    name: &'static str,
}

impl MemoryArea {
    pub fn new(start_addr: VirtAddr, end_addr: VirtAddr, flags: MemoryAttr, name: &'static str) -> Self {
        assert!(start_addr <= end_addr, "invalid memory area");
        MemoryArea { start_addr, end_addr, phys_start_addr: None, flags, name }
    }

    /// Create a new memory area which is identically mapped.
    /// 
    /// *notice that mappings will be done only when pushed into MemorySet*
    pub fn new_identity(start_addr: VirtAddr, end_addr: VirtAddr, flags: MemoryAttr, name: &'static str) -> Self {
        assert!(start_addr <= end_addr, "invalid memory area");
        MemoryArea { start_addr, end_addr, phys_start_addr: Some(start_addr), flags, name }
    }

    /// Create a new memory area mapped with a offset.
    /// 
    ///     (phys_start_addr + offset, phys_end_addr + offset)
    /// 
    /// is mapped to
    /// 
    ///     (phys_start_addr, phys_end_addr)
    /// 
    /// *notice that mappings will be done only when pushed into MemorySet*
    pub fn new_physical(phys_start_addr: PhysAddr, phys_end_addr: PhysAddr, offset: usize, flags: MemoryAttr, name: &'static str) -> Self {
        let start_addr = phys_start_addr + offset;
        let end_addr = phys_end_addr + offset;
        assert!(start_addr <= end_addr, "invalid memory area");
        let phys_start_addr = Some(phys_start_addr);
        MemoryArea { start_addr, end_addr, phys_start_addr, flags, name }
    }

    /// Get raw content in the area as a slice.
    pub unsafe fn as_slice(&self) -> &[u8] {
        use core::slice;
        slice::from_raw_parts(self.start_addr as *const u8, self.end_addr - self.start_addr)
    }

    /// Get raw content in the area as a mut slice.
    pub unsafe fn as_slice_mut(&self) -> &mut [u8] {
        use core::slice;
        slice::from_raw_parts_mut(self.start_addr as *mut u8, self.end_addr - self.start_addr)
    }

    /// If a virtual address is contained in the area.
    pub fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.start_addr && addr < self.end_addr
    }

    /// If two areas overlap with each other.
    fn is_overlap_with(&self, other: &MemoryArea) -> bool {
        let p0 = Page::of_addr(self.start_addr);
        let p1 = Page::of_addr(self.end_addr - 1) + 1;
        let p2 = Page::of_addr(other.start_addr);
        let p3 = Page::of_addr(other.end_addr - 1) + 1;
        !(p1 <= p2 || p0 >= p3)
    }

    /// Maps memory area to corresponding physical area.
    /// 
    /// If physical address is not specified, then maps to an allocated frame.
    fn map<T: InactivePageTable>(&self, pt: &mut T::Active) {
        match self.phys_start_addr {
            Some(phys_start) => {
                for page in Page::range_of(self.start_addr, self.end_addr) {
                    let addr = page.start_address();
                    let target = page.start_address() - self.start_addr + phys_start;
                    self.flags.apply(pt.map(addr, target));
                }
            }
            None => {
                for page in Page::range_of(self.start_addr, self.end_addr) {
                    let addr = page.start_address();
                    let target = T::alloc_frame().expect("failed to allocate frame");
                    self.flags.apply(pt.map(addr, target));
                }
            }
        }
    }

    /// Unmaps the memory area.
    fn unmap<T: InactivePageTable>(&self, pt: &mut T::Active) {
        for page in Page::range_of(self.start_addr, self.end_addr) {
            let addr = page.start_address();
            if self.phys_start_addr.is_none() {
                let target = pt.get_entry(addr).target();
                T::dealloc_frame(target);
            }
            pt.unmap(addr);
        }
    }
}

/// Attributes of a memory area.
/// 
/// Only simpliest functions are provided,
/// because attribute of a memory area rarely changes.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct MemoryAttr {
    user: bool,
    readonly: bool,
    execute: bool,
    hide: bool,
}

impl MemoryAttr {
    pub fn user(mut self) -> Self {
        self.user = true;
        self
    }
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }
    pub fn execute(mut self) -> Self {
        self.execute = true;
        self
    }
    pub fn hide(mut self) -> Self {
        self.hide = true;
        self
    }

    /// Apply attributes to a page entry.
    fn apply(&self, entry: &mut impl Entry) {
        if self.user { entry.set_user(true); }
        if self.readonly { entry.set_writable(false); }
        if self.execute { entry.set_execute(true); }
        if self.hide { entry.set_present(false); }
        if self.user || self.readonly || self.execute || self.hide { entry.update(); }
    }
}

/// 内存空间集合，包含若干段连续空间
/// 对应ucore中 `mm_struct`
pub struct MemorySet<T: InactivePageTable> {
    areas: Vec<MemoryArea>,
    page_table: T,
    kstack: Stack,
}

impl<T: InactivePageTable> MemorySet<T> {
    pub fn new() -> Self {
        MemorySet {
            areas: Vec::<MemoryArea>::new(),
            page_table: T::new(),
            kstack: T::alloc_stack(),
        }
    }
    /// Used for remap_kernel() where heap alloc is unavailable
    pub unsafe fn new_from_raw_space(slice: &mut [u8], kstack: Stack) -> Self {
        use core::mem::size_of;
        let cap = slice.len() / size_of::<MemoryArea>();
        MemorySet {
            areas: Vec::<MemoryArea>::from_raw_parts(slice.as_ptr() as *mut MemoryArea, 0, cap),
            page_table: T::new_bare(),
            kstack,
        }
    }
    
    /// Returns the MemoryArea containing a certain virtual address.
    pub fn find_area(&self, addr: VirtAddr) -> Option<&MemoryArea> {
        self.areas.iter().find(|area| area.contains(addr))
    }

    /// Adds a memory area to MemorySet and maps it.
    pub fn push(&mut self, area: MemoryArea) {
        assert!(self.areas.iter()
                    .find(|other| area.is_overlap_with(other))
                    .is_none(), "memory area overlap");
        self.page_table.edit(|pt| area.map::<T>(pt));
        self.areas.push(area);
    }

    /// Iterator implementation for for-loop.
    pub fn iter(&self) -> impl Iterator<Item=&MemoryArea> {
        self.areas.iter()
    }

    /// See `InactivePageTable.with`
    pub unsafe fn with(&self, f: impl FnOnce()) {
        self.page_table.with(f);
    }

    /// See `InactivePageTable.activate`
    pub unsafe fn activate(&self) {
        self.page_table.activate();
    }

    /// See `InactivePageTable.token`
    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    /// Returns address of kernel stack top.
    pub fn kstack_top(&self) -> usize {
        self.kstack.top
    }

    /// Unmaps all area, release all memories occupied.
    pub fn clear(&mut self) {
        let Self { ref mut page_table, ref mut areas, .. } = self;
        page_table.edit(|pt| {
            for area in areas.iter() {
                area.unmap::<T>(pt);
            }
        });
        areas.clear();
    }
}

impl<T: InactivePageTable> Clone for MemorySet<T> {
    fn clone(&self) -> Self {
        let mut page_table = T::new();
        page_table.edit(|pt| {
            for area in self.areas.iter() {
                area.map::<T>(pt);
            }
        });
        MemorySet {
            areas: self.areas.clone(),
            page_table,
            kstack: T::alloc_stack(),
        }
    }
}

impl<T: InactivePageTable> Drop for MemorySet<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T: InactivePageTable> Debug for MemorySet<T> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.debug_list()
            .entries(self.areas.iter())
            .finish()
    }
}

#[derive(Debug)]
pub struct Stack {
    pub top: usize,
    pub bottom: usize,
}