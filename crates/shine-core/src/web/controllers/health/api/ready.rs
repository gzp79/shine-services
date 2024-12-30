
#[utoipa::path(
    get, 
    path = "/info/ready", 
    tag = "health",
    description = "Health check.",
    responses(
        (status = OK, description = "Healthy.")
    )
)]
pub async fn get_ready() -> String {
    "Ok".into()
}
