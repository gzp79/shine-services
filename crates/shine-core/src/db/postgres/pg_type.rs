use tokio_postgres::types::Type as PGType;
use uuid::Uuid;

pub trait ToPGType {
    const PG_TYPE: PGType;
}

impl<T> ToPGType for Option<T>
where
    T: ToPGType,
{
    const PG_TYPE: PGType = <T as ToPGType>::PG_TYPE;
}

impl ToPGType for i16 {
    const PG_TYPE: PGType = PGType::INT2;
}

impl ToPGType for i32 {
    const PG_TYPE: PGType = PGType::INT4;
}

impl ToPGType for Uuid {
    const PG_TYPE: PGType = PGType::UUID;
}

impl ToPGType for &str {
    const PG_TYPE: PGType = PGType::VARCHAR;
}
