mod email_send_error;
mod email_sender;

pub mod smtp;

pub use self::{
    email_send_error::EmailSenderError,
    email_sender::{Email, EmailContent, EmailSender},
};
