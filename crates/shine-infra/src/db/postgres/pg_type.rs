use chrono::{DateTime, Utc};
use tokio_postgres::types::Type as PgType;
use uuid::Uuid;

pub trait PgValue: 'static {
    const PG_TYPE: PgType;
}

#[derive(Debug)]
pub struct PgValueTypeBool;
impl PgValue for PgValueTypeBool {
    const PG_TYPE: PgType = PgType::BOOL;
}

#[derive(Debug)]
pub struct PgValueTypeInt2;
impl PgValue for PgValueTypeInt2 {
    const PG_TYPE: PgType = PgType::INT2;
}

#[derive(Debug)]
pub struct PgValueTypeInt4;
impl PgValue for PgValueTypeInt4 {
    const PG_TYPE: PgType = PgType::INT4;
}

#[derive(Debug)]
pub struct PgValueTypeInt8;
impl PgValue for PgValueTypeInt8 {
    const PG_TYPE: PgType = PgType::INT8;
}

#[derive(Debug)]
pub struct PgValueTypeTimestamptz;
impl PgValue for PgValueTypeTimestamptz {
    const PG_TYPE: PgType = PgType::TIMESTAMPTZ;
}

#[derive(Debug)]
pub struct PgValueTypeUuid;
impl PgValue for PgValueTypeUuid {
    const PG_TYPE: PgType = PgType::UUID;
}

#[derive(Debug)]
pub struct PgValueTypeVarchar;
impl PgValue for PgValueTypeVarchar {
    const PG_TYPE: PgType = PgType::VARCHAR;
}

#[derive(Debug)]
pub struct PgValueTypeBytea;
impl PgValue for PgValueTypeBytea {
    const PG_TYPE: PgType = PgType::BYTEA;
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct PgValueTypeVarcharArray;
impl PgValue for PgValueTypeVarcharArray {
    const PG_TYPE: PgType = PgType::VARCHAR_ARRAY;
}
impl PgValue for &'static [PgValueTypeVarchar] {
    const PG_TYPE: PgType = PgType::VARCHAR_ARRAY;
}

#[allow(non_camel_case_types)]
pub struct PgValueTypeInt2Array;
impl PgValue for PgValueTypeInt2Array {
    const PG_TYPE: PgType = PgType::INT2_ARRAY;
}
impl PgValue for &'static [PgValueTypeInt2] {
    const PG_TYPE: PgType = PgType::INT2_ARRAY;
}

#[allow(non_camel_case_types)]
pub struct PgValueTypeInt8Array;
impl PgValue for PgValueTypeInt8Array {
    const PG_TYPE: PgType = PgType::INT8_ARRAY;
}
impl PgValue for &'static [PgValueTypeInt8] {
    const PG_TYPE: PgType = PgType::INT8_ARRAY;
}

#[allow(non_camel_case_types)]
pub struct PgValueTypeUuidArray;
impl PgValue for PgValueTypeUuidArray {
    const PG_TYPE: PgType = PgType::UUID_ARRAY;
}
impl PgValue for &'static [PgValueTypeUuid] {
    const PG_TYPE: PgType = PgType::UUID_ARRAY;
}

/// Marks a type whose Postgres OID is not a fixed builtin constant — a composite (`CREATE TYPE ...
/// AS (...)`) is the main case, since its OID is assigned per-database at creation time, not known
/// at compile time. `Type::UNKNOWN` tells Postgres to infer the real type itself.
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct PgValueTypeUnknown;
impl PgValue for PgValueTypeUnknown {
    const PG_TYPE: PgType = PgType::UNKNOWN;
}
impl PgValue for &'static [PgValueTypeUnknown] {
    const PG_TYPE: PgType = PgType::UNKNOWN;
}

pub trait ToPgType {
    type PgValueType: PgValue;
    const PG_TYPE: PgType = <Self::PgValueType as PgValue>::PG_TYPE;
}

impl<T> ToPgType for Option<T>
where
    T: ToPgType,
{
    type PgValueType = T::PgValueType;
}

impl ToPgType for bool {
    type PgValueType = PgValueTypeBool;
}

impl ToPgType for i16 {
    type PgValueType = PgValueTypeInt2;
}

impl ToPgType for i32 {
    type PgValueType = PgValueTypeInt4;
}

impl ToPgType for i64 {
    type PgValueType = PgValueTypeInt8;
}

impl ToPgType for DateTime<Utc> {
    type PgValueType = PgValueTypeTimestamptz;
}

impl ToPgType for Uuid {
    type PgValueType = PgValueTypeUuid;
}

impl ToPgType for &str {
    type PgValueType = PgValueTypeVarchar;
}

impl ToPgType for Vec<u8> {
    type PgValueType = PgValueTypeBytea;
}

impl ToPgType for &[u8] {
    type PgValueType = PgValueTypeBytea;
}

impl<T> ToPgType for &[T]
where
    T: ToPgType,
    &'static [T::PgValueType]: PgValue,
{
    type PgValueType = &'static [T::PgValueType];
}
