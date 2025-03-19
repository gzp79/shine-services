use tokio_postgres::types::Type as PGType;
use uuid::Uuid;

pub trait PGValue: 'static {
    const PG_TYPE: PGType;
}

#[derive(Debug)]
pub struct PGValueTypeBOOL;
impl PGValue for PGValueTypeBOOL {
    const PG_TYPE: PGType = PGType::BOOL;
}

#[derive(Debug)]
pub struct PGValueTypeINT2;
impl PGValue for PGValueTypeINT2 {
    const PG_TYPE: PGType = PGType::INT2;
}

#[derive(Debug)]
pub struct PGValueTypeINT4;
impl PGValue for PGValueTypeINT4 {
    const PG_TYPE: PGType = PGType::INT4;
}

#[derive(Debug)]
pub struct PGValueTypeUUID;
impl PGValue for PGValueTypeUUID {
    const PG_TYPE: PGType = PGType::UUID;
}

#[derive(Debug)]
pub struct PGValueTypeVARCHAR;
impl PGValue for PGValueTypeVARCHAR {
    const PG_TYPE: PGType = PGType::VARCHAR;
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct PGValueTypeVARCHAR_ARRAY;
impl PGValue for PGValueTypeVARCHAR_ARRAY {
    const PG_TYPE: PGType = PGType::VARCHAR_ARRAY;
}
impl PGValue for &'static [PGValueTypeVARCHAR] {
    const PG_TYPE: PGType = PGType::VARCHAR_ARRAY;
}

#[allow(non_camel_case_types)]
pub struct PGValueTypeINT2_ARRAY;
impl PGValue for PGValueTypeINT2_ARRAY {
    const PG_TYPE: PGType = PGType::INT2_ARRAY;
}
impl PGValue for &'static [PGValueTypeINT2] {
    const PG_TYPE: PGType = PGType::INT2_ARRAY;
}

pub trait ToPGType {
    type PGValueType: PGValue;
    const PG_TYPE: PGType = <Self::PGValueType as PGValue>::PG_TYPE;
}

impl<T> ToPGType for Option<T>
where
    T: ToPGType,
{
    type PGValueType = T::PGValueType;
}

impl ToPGType for bool {
    type PGValueType = PGValueTypeBOOL;
}

impl ToPGType for i16 {
    type PGValueType = PGValueTypeINT2;
}

impl ToPGType for i32 {
    type PGValueType = PGValueTypeINT4;
}

impl ToPGType for Uuid {
    type PGValueType = PGValueTypeUUID;
}

impl ToPGType for &str {
    type PGValueType = PGValueTypeVARCHAR;
}

impl<T> ToPGType for &[T]
where
    T: ToPGType,
    &'static [T::PGValueType]: PGValue,
{
    type PGValueType = &'static [T::PGValueType];
}
