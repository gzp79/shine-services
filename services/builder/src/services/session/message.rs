use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum Message {
    Chat(Uuid, String),
}
