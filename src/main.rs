use anyhow::Context;
use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::{
    collections::HashMap,
    env,
    sync::{Arc, RwLock},
};
use thiserror::Error;

// 起こりうるエラーの定義
#[derive(Debug, Error)]
enum RepositoryError {
    #[error("NotFound id is {0}")]
    NotFound(i32),
}

// トレイトで共通の振る舞い（CRUD）を定義する
// これを継承した構造体はCRUDできるようになる
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    fn create(&self, payload: CreateTodo) -> Todo;
    fn find(&self, id: i32) -> Option<Todo>;
    fn all(&self) -> Vec<Todo>;
    fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo>;
    fn delete(&self, id: i32) -> anyhow::Result<Todo>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Todo {
    id: i32,
    text: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CreateTodo {
    text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateTodo {
    text: Option<String>,
    completed: bool,
}

impl Todo {
    pub fn new(id: i32, text: String) -> Self {
        Self {
            id,
            text,
            completed: false,
        }
    }
}

// とりあえずTODOデータをHashMapに保存する
type TodoDatas = HashMap<i32, Todo>;

#[derive(Debug, Clone)]
pub struct TodoRepositoryForMemory {
    store: Arc<RwLock<TodoDatas>>,
}

impl TodoRepositoryForMemory {
    pub fn new() -> Self {
        TodoRepositoryForMemory {
            store: Arc::default(),
        }
    }
}

impl TodoRepository for TodoRepositoryForMemory {
    fn create(&self, payload: CreateTodo) -> Todo {
        todo!()
    }

    fn find(&self, id: i32) -> Option<Todo> {
        todo!()
    }

    fn all(&self) -> Vec<Todo> {
        todo!()
    }

    fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo> {
        todo!()
    }

    fn delete(&self, id: i32) -> anyhow::Result<Todo> {
        todo!()
    }
}

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

pub async fn create_todo<T: TodoRepository>(
    Json(payload): Json<CreateTodo>,
    Extension(repository): Extension<Arc<T>>
) -> impl IntoResponse {
    let todo = repository.create(payload);

    (StatusCode::CREATED, Json(todo))
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
