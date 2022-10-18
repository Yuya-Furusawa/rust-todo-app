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

    let app = create_app();
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

// ルーティング設定の作成
fn create_app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/users", post(create_user))
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
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct CreateUser {
    username: String,
}

// 構造体をJSONにシリアライズすることが可能になる
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct User {
    id: u64,
    username: String,
}

// テストを書く
// cfg(test)なのでプロダクションコードからは削除される
#[cfg(test)]
mod test {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Method, Request},
    };
    use tower::ServiceExt;

    // root関数のテスト
    #[tokio::test]
    async fn should_return_hello_world() {
        // リクエストを作成
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        // 作ったリクエストからoneshot関数でレスポンスを得る
        let res = create_app().oneshot(req).await.unwrap();

        // 得られたレスポンスをBytes型を経てString型に変換する
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "Hello, world!")
    }

    // JSON bodyのテスト
    #[tokio::test]
    async fn should_return_user_data() {
        // 同様にリクエストを作成し、レスポンスを得て、User型インスタンスを作る
        // ちゃんとheaderとbodyも設定しておく
        let req = Request::builder()
            .uri("/users")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(r#"{ "username": "田中太郎" }"#))
            .unwrap();
        let res = create_app().oneshot(req).await.unwrap();

        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let user: User = serde_json::from_str(&body).expect("cannot convert User instance.");
        assert_eq!(
            user,
            User {
                id: 1337,
                username: "田中太郎".to_string(),
            }
        )
    }
}
