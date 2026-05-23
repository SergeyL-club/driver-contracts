pub mod types;
pub use types::*;

mod functions;

#[unsafe(no_mangle)]
pub extern "C" fn abi_string_copy(src: AbiString) -> AbiString {
    AbiString::copy_from(&src)
}

#[unsafe(no_mangle)]
pub extern "C" fn abi_string_destroy(str_to_destroy: AbiString) {
    str_to_destroy.destroy();
}

#[unsafe(no_mangle)]
pub extern "C" fn memory_meta_type(type_base: TypeBase) -> TypeMeta {
    type_base.memory_meta()
}

#[unsafe(no_mangle)]
pub extern "C" fn struct_builder_create(name: AbiString) -> *mut StructBuilder {
    StructBuilder::new(name)
}

#[unsafe(no_mangle)]
pub extern "C" fn struct_builder_add_field(
    builder: *mut StructBuilder,
    field_name: FieldName,
    field_type: TypeBase,
) {
    if let Some(b) = unsafe { builder.as_mut() } {
        b.add_field(field_name, field_type);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn struct_builder_remove_field(
    builder: *mut StructBuilder,
    name_to_remove: AbiString,
) -> bool {
    if let Some(b) = unsafe { builder.as_mut() } {
        b.remove_field(name_to_remove)
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn struct_builder_clear_fields(builder: *mut StructBuilder) {
    if let Some(b) = unsafe { builder.as_mut() } {
        b.clear_fields();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn struct_builder_destroy(builder: *mut StructBuilder) {
    if !builder.is_null() {
        unsafe {
            let b = Box::from_raw(builder);
            b.destroy();
        }
    }
}
