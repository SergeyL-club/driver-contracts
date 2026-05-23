mod lib; // Импортируем типы контракта
use lib::{AbiString, StructBuilder, TypeBase};

unsafe extern "C" {
    fn struct_builder_create(name: AbiString) -> *mut StructBuilder;
    fn struct_builder_add_field(
        builder: *mut StructBuilder,
        field_name: AbiString,
        field_type: TypeBase,
    );
    fn struct_builder_remove_field(builder: *mut StructBuilder, name_to_remove: AbiString) -> bool;
    fn struct_builder_clear_fields(builder: *mut StructBuilder);
    fn struct_builder_destroy(builder: *mut StructBuilder);
}

pub struct SafeStructBuilder {
    raw: *mut StructBuilder,
}

impl SafeStructBuilder {
    pub fn new(name: &str) -> Option<Self> {
        let abi_str = AbiString(name.as_ptr(), name.len());
        let raw = unsafe { struct_builder_create(abi_str) };
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    pub fn add_field(&mut self, name: &str, field_type: TypeBase) {
        let abi_str = AbiString(name.as_ptr(), name.len());
        unsafe { struct_builder_add_field(self.raw, abi_str, field_type) };
    }

    pub fn remove_field(&mut self, name: &str) -> bool {
        let abi_str = AbiString(name.as_ptr(), name.len());
        unsafe { struct_builder_remove_field(self.raw, abi_str) }
    }

    pub fn clear_fields(&mut self) {
        unsafe { struct_builder_clear_fields(self.raw) };
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
        unsafe { struct_builder_destroy(self.raw) };
    }
}
