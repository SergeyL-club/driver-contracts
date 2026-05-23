use crate::*;

impl AbiString {
    pub fn copy_from(src: &Self) -> Self {
        if src.0.is_null() {
            return Self(std::ptr::null(), 0);
        }
        unsafe {
            let c_bytes = std::slice::from_raw_parts(src.0, src.1);
            let rust_copied_bytes = c_bytes.to_vec().into_boxed_slice();
            let len = rust_copied_bytes.len();
            let raw_ptr = Box::into_raw(rust_copied_bytes) as *const u8;
            Self(raw_ptr, len)
        }
    }

    pub fn destroy(self) {
        if !self.0.is_null() {
            unsafe {
                let raw_slice_ptr = std::ptr::slice_from_raw_parts_mut(self.0 as *mut u8, self.1);
                let _ = Box::from_raw(raw_slice_ptr);
            }
        }
    }
}

impl TypeBase {
    pub fn memory_meta(&self) -> TypeMeta {
        match *self {
            Self::INT32 | Self::UINT32 | Self::FLOAT32 => TypeMeta(4, 4),
            Self::INT64 | Self::UINT64 | Self::FLOAT64 => TypeMeta(8, 8),
            Self::PTR(_) => {
                let ptr_size = std::mem::size_of::<usize>();
                TypeMeta(ptr_size, ptr_size)
            }
            Self::STRING(_) => {
                let ptr_size = std::mem::size_of::<usize>();
                TypeMeta(ptr_size * 2, ptr_size)
            }
            Self::STRUCT(AbiStruct(_, size, align)) => TypeMeta(size, align),
        }
    }
}

impl StructBuilder {
    unsafe fn take_fields_vec(&mut self) -> Vec<FieldElementBuilder> {
        if self.fields.0.is_null() {
            Vec::new()
        } else {
            unsafe {
                Vec::from_raw_parts(
                    self.fields.0 as *mut FieldElementBuilder,
                    self.fields.1,
                    self.fields.1,
                )
            }
        }
    }
    unsafe fn save_fields_vec(&mut self, mut vec: Vec<FieldElementBuilder>) {
        vec.shrink_to_fit();
        self.fields.1 = vec.len();
        self.fields.0 = if vec.is_empty() {
            std::ptr::null()
        } else {
            vec.as_ptr()
        };
        std::mem::forget(vec);
    }

    pub fn recalculate_layout(&mut self) {
        unsafe {
            let mut fields = self.take_fields_vec();

            self.size = 0;
            self.align = 1;

            for field in &mut fields {
                let meta = field.2.memory_meta();
                let field_size = meta.0;
                let field_align = meta.1;

                if field_align > self.align {
                    self.align = field_align;
                }

                let current_offset = (self.size + field_align - 1) & !(field_align - 1);
                field.0 = current_offset; // Записываем вычисленный FieldOffset в поле

                self.size = current_offset + field_size;
            }

            self.size = (self.size + self.align - 1) & !(self.align - 1);

            self.save_fields_vec(fields);
        }
    }
}

impl StructBuilder {
    pub fn new(name: AbiString) -> *mut Self {
        if name.0.is_null() {
            return std::ptr::null_mut();
        }

        let name_copied = AbiString::copy_from(&name);

        let builder = Self {
            name: name_copied,
            size: 0,
            align: 1,
            fields: FieldStructBuilder(std::ptr::null(), 0),
        };
        Box::into_raw(Box::new(builder))
    }

    pub fn add_field(&mut self, field_name: FieldName, field_type: TypeBase) {
        if field_name.0.is_null() {
            return;
        }

        let mut fields = unsafe { self.take_fields_vec() };

        let field_name_copied = AbiString::copy_from(&field_name);

        let new_field = FieldElementBuilder(0, field_name_copied, field_type);
        fields.push(new_field);

        unsafe { self.save_fields_vec(fields) };

        self.recalculate_layout();
    }

    pub fn remove_field(&mut self, name_to_remove: AbiString) -> bool {
        if name_to_remove.0.is_null() {
            return false;
        }

        let mut fields = unsafe { self.take_fields_vec() };
        let target_bytes =
            unsafe { std::slice::from_raw_parts(name_to_remove.0, name_to_remove.1) };

        let position = fields.iter().position(|f| {
            let f_name_bytes = unsafe { std::slice::from_raw_parts(f.1.0, f.1.1) };
            f_name_bytes == target_bytes
        });

        if let Some(idx) = position {
            let removed_field = fields.remove(idx);

            removed_field.1.destroy();

            unsafe { self.save_fields_vec(fields) };

            self.recalculate_layout();
            true
        } else {
            unsafe { self.save_fields_vec(fields) };
            false
        }
    }

    pub fn clear_fields(&mut self) {
        let fields = unsafe { self.take_fields_vec() };

        for field in fields {
            field.1.destroy();
        }
        // Сам массив полей автоматически очистится здесь (Drop вектора)

        self.fields = FieldStructBuilder(std::ptr::null(), 0);
        self.size = 0;
        self.align = 1;
    }

    pub fn destroy(self) {
        let mut mutable_self = self;
        mutable_self.clear_fields();
        mutable_self.name.destroy();
    }
}
