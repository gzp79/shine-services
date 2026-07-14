use uuid::Uuid;

pub const MAX_SEARCH_RESULT_COUNT: usize = 100;

#[derive(Debug)]
pub struct SearchIdentity<'a> {
    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
    pub count: Option<usize>,
}
