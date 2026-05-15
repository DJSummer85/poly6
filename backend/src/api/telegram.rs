use axum::{
    extract::State,
    response::{IntoResponse, Json, Response},
    Extension,
};
use serde_json::json;
use crate::middleware::auth::Claims;
use super::AppState;

pub async fn test_telegram(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;
    
    // Először ürítsük a cache-t, hogy friss kulcsokkal próbálkozzon
    state.telegram_service.invalidate_cache(user_id).await;

    match state.telegram_service.send_message(user_id, "🔔 <b>Polymarket Bot</b>\n\nTeszt üzenet sikeres! Az értesítések beállítva.").await {
        Ok(_) => Json(json!({ "success": true, "message": "Teszt üzenet elküldve!" })).into_response(),
        Err(e) => Json(json!({ "success": false, "error": e.to_string() })).into_response(),
    }
}
