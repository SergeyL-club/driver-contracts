pub mod types;
use std::sync::Arc;
use types::{AbiString, StructBuilder, TypeBase};

#[derive(Clone)]
pub struct CoreVTable {
    pub struct_builder_create: unsafe extern "C" fn(name: AbiString) -> *mut StructBuilder,
    pub struct_builder_add_field:
        unsafe extern "C" fn(builder: *mut StructBuilder, name: AbiString, field_type: TypeBase),
    pub struct_builder_remove_field:
        unsafe extern "C" fn(builder: *mut StructBuilder, name: AbiString) -> bool,
    pub struct_builder_clear_fields: unsafe extern "C" fn(builder: *mut StructBuilder),
    pub struct_builder_destroy: unsafe extern "C" fn(builder: *mut StructBuilder),
}

pub struct SafeStructBuilder {
    raw: *mut StructBuilder,
    vtable: Arc<CoreVTable>,
}

impl SafeStructBuilder {
    pub fn into_raw(self) -> *mut StructBuilder {
        let raw = self.raw;
        std::mem::forget(self);
        raw
    }

    pub unsafe fn from_raw(raw: *mut StructBuilder, vtable: Arc<CoreVTable>) -> Self {
        Self { raw, vtable }
    }
}

impl SafeStructBuilder {
    pub fn new(name: &str, vtable: Arc<CoreVTable>) -> Option<Self> {
        let abi_str = AbiString(name.as_ptr(), name.len());
        let raw = unsafe { (vtable.struct_builder_create)(abi_str) };

        if raw.is_null() {
            None
        } else {
            Some(Self { raw, vtable })
        }
    }

    pub fn add_field(&mut self, name: &str, field_type: TypeBase) {
        let abi_str = AbiString(name.as_ptr(), name.len());
        unsafe { (self.vtable.struct_builder_add_field)(self.raw, abi_str, field_type) };
    }

    pub fn remove_field(&mut self, name: &str) -> bool {
        let abi_str = AbiString(name.as_ptr(), name.len());
        unsafe { (self.vtable.struct_builder_remove_field)(self.raw, abi_str) }
    }

    pub fn clear_fields(&mut self) {
        unsafe { (self.vtable.struct_builder_clear_fields)(self.raw) };
    }

    pub fn get_layout(&self) -> (usize, usize) {
        unsafe {
            let b = &*self.raw;
            (b.size, b.align)
        }
    }
}

impl Drop for SafeStructBuilder {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.struct_builder_destroy)(self.raw);
        };
    }
}
