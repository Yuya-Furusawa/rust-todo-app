mod handlers;
mod repositories;

use crate::repositories::{TodoRepository, TodoRepositoryForMemory};
use axum::{
    extract::Extension,
    routing::{get, post},
    Router
};
use std::net::SocketAddr;
use std::{
    env,
    sync::{Arc},
};

use handlers::create_todo;

#[tokio::main]
async fn main() {
    // loggingの初期化
    let log_lavel = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_lavel);
    tracing_subscriber::fmt::init();

    let repository = TodoRepositoryForMemory::new();
    let app = create_app(repository);

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
// 柔軟性をもたせるために、TodoRepositoryトレイトを継承したジェネリクスで引数を型指定
fn create_app<T: TodoRepository>(repository: T) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/todos", post(create_todo::<T>))
        .layer(Extension(Arc::new(repository))) // axumアプリ内でrepositoryを共有できるようになる
}

async fn root() -> &'static str {
    "Hello, world!"
}

// テストを書く
// cfg(test)なのでプロダクションコードからは削除される
#[cfg(test)]
mod test {
    use super::*;
    use axum::{
        body::Body,
        http::Request,
    };
    use tower::ServiceExt;

    // root関数のテスト
    #[tokio::test]
    async fn should_return_hello_world() {
        let repository = TodoRepositoryForMemory::new();
        // リクエストを作成
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        // 作ったリクエストからoneshot関数でレスポンスを得る
        let res = create_app(repository).oneshot(req).await.unwrap();

        // 得られたレスポンスをBytes型を経てString型に変換する
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "Hello, world!")
    }

}
