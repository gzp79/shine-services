use shine_infra::email::Email;

#[derive(Clone, Debug)]
pub struct ExternalUserInfo {
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<Email>,
}

impl ExternalUserInfo {
    /// Normalize external user info (e.g., truncate long names, normalize emails)
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if let Some(name) = &self.name {
            if name.chars().count() > 20 {
                let truncated_name: String = name.chars().take(20).collect();
                log::info!("Truncating name from '{name}' to '{truncated_name}'");
                self.name = Some(truncated_name);
            }
        }

        self
    }
}
