use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::env;

#[tokio::main]
async fn main() {
    // loggingの初期化
    let log_lavel = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_lavel);
    tracing_subscriber::fmt::init();

    // ルーティング設定の作成
    let app = Router::new()
        .route("/", get(root))
        .route("/users", post(create_user));
    // アドレスを作成する
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    // ログ情報の出力
    tracing::debug!("listening on {}", addr);

    // アドレスをサーバーにバインドし、サーバーを立ち上げる
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await // 非同期タスクはawaitされて初めて実行される
        .unwrap();
}

async fn root() -> &'static str {
    "Hello, world!"
}

async fn create_user(Json(payload): Json<CreateUser>) -> impl IntoResponse {
    let user = User {
        id: 1337,
        username: payload.username,
    };

    (StatusCode::CREATED, Json(user))
}

// JSONを構造体にデシリアライズすることが可能になる
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// 構造体をJSONにシリアライズすることが可能になる
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}
