use uuid::Uuid;

pub const MAX_SEARCH_RESULT_COUNT: usize = 100;

#[derive(Debug)]
pub enum SearchIdentityOrder {
    UserId(Option<Uuid>),
    Email(Option<(String, Uuid)>),
    Name(Option<(String, Uuid)>),
}

#[derive(Debug)]
pub struct SearchIdentity<'a> {
    pub order: SearchIdentityOrder,
    pub count: Option<usize>,

    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
}
