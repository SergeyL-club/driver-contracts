pub type TypeSize = usize;
pub type TypeAlign = usize;
pub type TypePtr = *const u8;

#[repr(C)]
#[allow(dead_code)]
pub struct TypeMeta(pub TypeSize, pub TypeAlign);

#[repr(C)]
pub struct AbiString(pub TypePtr, pub TypeSize);

#[repr(C)]
pub struct AbiStruct(pub TypePtr, pub TypeSize, pub TypeAlign);

#[repr(C)]
#[allow(dead_code)]
pub enum TypeBase {
    INT32,
    INT64,
    UINT32,
    UINT64,
    FLOAT32,
    FLOAT64,
    PTR(TypePtr),
    STRING(AbiString),
    STRUCT(AbiStruct),
}

pub type FieldOffset = usize;
pub type FieldName = AbiString;
pub type FieldSize = usize;

pub type StructSize = usize;

#[repr(C)]
pub struct FieldElementBuilder(pub FieldOffset, pub FieldName, pub TypeBase);

#[repr(C)]
pub struct FieldStructBuilder(pub *const FieldElementBuilder, pub FieldSize);

#[repr(C)]
pub struct StructBuilder {
    pub name: AbiString,
    pub size: StructSize,
    pub align: TypeAlign,
    pub fields: FieldStructBuilder,
}
