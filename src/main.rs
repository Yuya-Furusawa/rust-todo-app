mod handlers;
mod repositories;

use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use sqlx::PgPool;
use hyper::header::CONTENT_TYPE;
use tower_http::cors::{Any, CorsLayer, Origin};
use std::net::SocketAddr;
use std::{env, sync::Arc};

use handlers::{
    label::{all_label, create_label, delete_label},
    todo::{all_todo, create_todo, delete_todo, find_todo, update_todo},
};
use repositories::{
    label::{LabelRepository, LabelRepositoryForDb},
    todo::{TodoRepository, TodoRepositoryForDb},
};

#[tokio::main]
async fn main() {
    // loggingの初期化
    let log_lavel = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_lavel);
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
    tracing::debug!("start connect database...");
    let pool = PgPool::connect(database_url)
        .await
        .expect(&format!("fail connect database, url is [{}]", database_url));

    let app = create_app(
        TodoRepositoryForDb::new(pool.clone()),
        LabelRepositoryForDb::new(pool.clone()),
    );

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
fn create_app<Todo: TodoRepository, Label: LabelRepository>(
    todo_repository: Todo,
    label_repository: Label,
) -> Router {
    Router::new()
        .route("/", get(root))
        .route(
            "/todos",
            post(create_todo::<Todo>)
                .get(all_todo::<Todo>)
        )
        .route(
            "/todos/:id",
            get(find_todo::<Todo>)
                .delete(delete_todo::<Todo>)
                .patch(update_todo::<Todo>),
        )
        .route(
            "/labels",
            post(create_label::<Label>)
                .get(all_label::<Label>)
        )
        .route("/labels/:id", delete(delete_label::<Label>))
        .layer(Extension(Arc::new(todo_repository)))
        .layer(Extension(Arc::new(label_repository))) // axumアプリ内でrepositoryを共有できるようになる
        .layer(
            CorsLayer::new()
                .allow_origin(Origin::exact("http://localhost:3001".parse().unwrap()))
                .allow_methods(Any)
                .allow_headers(vec![CONTENT_TYPE])
        )
}

async fn root() -> &'static str {
    "Hello, world!"
}

// テストを書く
// cfg(test)なのでプロダクションコードからは削除される
#[cfg(test)]
mod test {
    use super::*;
    use crate::repositories::{
        todo::{test_utils::TodoRepositoryForMemory, CreateTodo, TodoEntity},
        label::{test_utils::LabelRepositoryForMemory, Label},
    };
    use axum::{
        body::Body,
        http::{header, Method, Request},
        response::Response,
    };
    use hyper::StatusCode;
    use tower::ServiceExt;

    // テスト用のlabelを作る関数
    fn label_fixture() -> (Vec<Label>, Vec<i32>) {
        let id = 999;
        (
            vec![
                Label {
                    id,
                    name: String::from("test label")
                },
            ],
            vec![id]
        )
    }

    // root関数のテスト
    #[tokio::test]
    async fn should_return_hello_world() {
        let (labels, _label_ids) = label_fixture();

        // リクエストを作成
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        // 作ったリクエストからoneshot関数でレスポンスを得る
        let res = create_app(
            TodoRepositoryForMemory::new(labels),
            LabelRepositoryForMemory::new()
        ).oneshot(req).await.unwrap();

        // 得られたレスポンスをBytes型を経てString型に変換する
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "Hello, world!")
    }

    // メソッドやボディを受けとり、リクエストを作る
    fn build_req_with_json(path: &str, method: Method, json_body: String) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(json_body))
            .unwrap()
    }

    // メソッドを受け取り、リクエストを作る（ボディは空）
    fn build_req_with_empty(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .body(Body::empty())
            .unwrap()
    }

    // レスポンスを受け取り、BodyをTodo型に変換する
    async fn res_to_todo(res: Response) -> TodoEntity {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: TodoEntity = serde_json::from_str(&body)
            .expect(&format!("cannot convert Todo instance. body: {}", body));
        todo
    }

    // レスポンスを受け取り、BodyをLabel型に変換する
    async fn res_to_label(res: Response) -> Label {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let label: Label = serde_json::from_str(&body)
            .expect(&format!("cannot convert Label instance. body: {}", body));
        label
    }

    #[tokio::test]
    async fn should_create_todo() {
        let (labels, _label_ids) = label_fixture();

        let expected = TodoEntity::new(1, "should_return_created_todo".to_string(), labels.clone());
        let req = build_req_with_json(
            "/todos",
            Method::POST,
            r#"{ "text": "should_return_created_todo", "labels": [999] }"#.to_string(),
        );
        // 疑似リクエストでリクエストの検証
        let res = create_app(
            TodoRepositoryForMemory::new(labels.clone()),
            LabelRepositoryForMemory::new(),
        ).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_create_label() {
        let (labels, _label_ids) = label_fixture();

        let expected = Label::new(1, "should_return_created_label".to_string());
        let req = build_req_with_json(
            "/labels",
            Method::POST,
            r#"{ "name": "should_return_created_label" }"#.to_string(),
        );
        let res = create_app(
            TodoRepositoryForMemory::new(labels.clone()),
            LabelRepositoryForMemory::new(),
        ).oneshot(req).await.unwrap();
        let label = res_to_label(res).await;
        assert_eq!(expected, label);
    }

    #[tokio::test]
    async fn should_find_todo() {
        let (labels, label_ids) = label_fixture();
        let expected = TodoEntity::new(1, "should_find_todo".to_string(), labels.clone());

        // repositoryを作成し、1件だけ保存してみる
        let repository = TodoRepositoryForMemory::new(labels.clone());
        repository
            .create(CreateTodo::new("should_find_todo".to_string(), label_ids.clone()))
            .await
            .expect("failed create todo");

        let req = build_req_with_empty(Method::GET, "/todos/1");
        let res = create_app(
            repository,
            LabelRepositoryForMemory::new(),
        ).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_get_all_todos() {
        let (labels, label_ids) = label_fixture();
        let expected = TodoEntity::new(1, "should_get_all_todos".to_string(), labels.clone());

        let repository = TodoRepositoryForMemory::new(labels.clone());
        repository
            .create(CreateTodo::new("should_get_all_todos".to_string(), label_ids.clone()))
            .await
            .expect("failed create todo");

        let req = build_req_with_empty(Method::GET, "/todos");
        let res = create_app(
            repository,
            LabelRepositoryForMemory::new(),
        ).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        // todoはベクトルになることに注意
        let todo: Vec<TodoEntity> = serde_json::from_str(&body)
            .expect(&format!("cannot convert Todo instance. body: {}", body));
        assert_eq!(vec![expected], todo);
    }

    #[tokio::test]
    async fn should_get_all_labels() {
        let (labels, _label_ids) = label_fixture();
        let expected = Label::new(1, "should_get_all_labels".to_string());

        let repository = LabelRepositoryForMemory::new();
        repository
            .create("should_get_all_labels".to_string())
            .await
            .expect("failed create label");

        let req = build_req_with_empty(Method::GET, "/labels");
        let res = create_app(
            TodoRepositoryForMemory::new(labels.clone()),
            repository,
        ).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let label: Vec<Label> = serde_json::from_str(&body)
            .expect(&format!("cannot convert Label instance. body: {}", body));
        assert_eq!(vec![expected], label);
    }

    #[tokio::test]
    async fn should_update_todo() {
        let (labels, label_ids) = label_fixture();
        let expected = TodoEntity::new(1, "should_update_todo".to_string(), labels.clone());

        let repository = TodoRepositoryForMemory::new(labels.clone());
        repository
            .create(CreateTodo::new("should_update_todo".to_string(), label_ids.clone()))
            .await
            .expect("failed create todo");

        let req = build_req_with_json(
            "/todos/1",
            Method::PATCH,
            r#"{
                "id": 1,
                "text": "should_update_todo",
                "completed": false
            }"#
            .to_string(),
        );
        let res = create_app(
            repository,
            LabelRepositoryForMemory::new(),
        ).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_delete_todo() {
        let (labels, label_ids) = label_fixture();

        let repository = TodoRepositoryForMemory::new(labels.clone());
        repository
            .create(CreateTodo::new("should_delete_todo".to_string(), label_ids.clone()))
            .await
            .expect("failed create todo");

        let req = build_req_with_empty(Method::DELETE, "/todos/1");
        let res = create_app(
            repository,
            LabelRepositoryForMemory::new(),
        ).oneshot(req).await.unwrap();
        assert_eq!(StatusCode::NO_CONTENT, res.status());
    }

    #[tokio::test]
    async fn should_delete_label() {
        let (labels, _label_ids) = label_fixture();

        let repository = LabelRepositoryForMemory::new();
        repository
            .create("should_delete_label".to_string())
            .await
            .expect("failed create label");

        let req = build_req_with_empty(Method::DELETE, "/labels/1");
        let res = create_app(
            TodoRepositoryForMemory::new(labels.clone()),
            repository,
        ).oneshot(req).await.unwrap();
        assert_eq!(StatusCode::NO_CONTENT, res.status());
    }
}
