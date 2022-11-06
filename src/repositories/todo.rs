use anyhow::Ok;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, FromRow};
use validator::Validate;

use super::{
    label::Label,
    RepositoryError,
};

// データレポジトリを作成

// トレイトを用いてデータレポジトリの振る舞い（CRUD）を定義する
// これを継承した構造体はCRUDできるようになる
// CloneとSendとSyncを実装した型に対して実装するトレイト（SendとSyncは基本的にどの型にも実装されている）
// 'staticとする事によってライフタイムをなくす
#[async_trait] // トレイトの各メソッドをasyncにする
pub trait TodoRepository: Clone + Send + Sync + 'static {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity>;
    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity>;
    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>>;
    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
struct TodoFromRow {
    id: i32,
    text: String,
    completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct TodoWithLabelFromRow {
    id: i32,
    text: String,
    completed: bool,
    label_id: Option<i32>,
    label_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TodoEntity {
    pub id: i32,
    pub text: String,
    pub completed: bool,
    pub labels: Vec<Label>,
}

// Vec<TodoWithLabelFromRow>からVec<TodoEntity>への変換
fn fold_entities(rows: Vec<TodoWithLabelFromRow>) -> Vec<TodoEntity> {
    let mut rows = rows.iter();
    let mut accum: Vec<TodoEntity> = vec![];
    'outer: while let Some(row) = rows.next() {
        let mut todos = accum.iter_mut();
        while let Some(todo) = todos.next() {
            if todo.id == row.id {
                todo.labels.push(
                    Label {
                        id: row.label_id.unwrap(),
                        name: row.label_name.clone().unwrap(),
                    }
                );
                continue 'outer;
            }
        }
        let labels = if row.label_id.is_some() {
            vec![
                Label {
                    id: row.label_id.unwrap(),
                    name: row.label_name.clone().unwrap(),
                }
            ]
        } else {
            vec![]
        };

        accum.push(
            TodoEntity {
                id: row.id,
                text: row.text.clone(),
                completed: row.completed,
                labels
            }
        );
    }
    accum
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct CreateTodo {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: String,
    labels: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct UpdateTodo {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: Option<String>,
    completed: Option<bool>,
    labels: Option<Vec<i32>>,
}

// データベースの操作を行うオブジェクト

#[derive(Debug, Clone)]
pub struct TodoRepositoryForDb {
    pool: PgPool,
}

impl TodoRepositoryForDb {
    pub fn new(pool: PgPool) -> Self {
        TodoRepositoryForDb { pool }
    }
}

#[async_trait]
impl TodoRepository for TodoRepositoryForDb {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity> {
        let tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, TodoFromRow>(
            r#"
                insert into todos (text, completed)
                values ($1, false)
                returning *
            "#,)
            .bind(payload.text.clone())
            .fetch_one(&self.pool)
            .await?;

        sqlx::query(
            r#"
                insert into todo_labels (todo_id, label_id)
                select $1, id
                from unnest($2) as t(id)
            "#,
        )
        .bind(row.id)
        .bind(payload.labels)
        .execute(&self.pool)
        .await?;

        tx.commit().await?;

        let todo = self.find(row.id).await?;
        Ok(todo)
    }

    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
        let items = sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                select todos.*, labels.id as label_id, labels.name as label_name from todos
                    left outer join todo_labels tl on todos.id = tl.todo_id
                    left outer join labels on labels.id = tl.label_id where todos.id=$1;
            "#
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        let todos = fold_entities(items);
        let todo = todos.first().ok_or(RepositoryError::NotFound(id))?;

        Ok(todo.clone())
    }

    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
        let items = sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                select todos.*, labels.id as label_id, labels.name as label_name from todos
                    left outer join todo_labels tl on todos.id = tl.todo_id
                    left outer join labels on labels.id = tl.labe_id order by todos.id desc;
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(fold_entities(items))
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
        let tx = self.pool.begin().await?;

        let old_todo = self.find(id).await?;
        sqlx::query_as::<_, TodoWithLabelFromRow>(
            r#"
                update todos set text=$1, completed=$2
                where id=$3
                returning *
            "#,
        )
        .bind(payload.text.unwrap_or(old_todo.text))
        .bind(payload.completed.unwrap_or(old_todo.completed))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if let Some(labels) = payload.labels {
            sqlx::query(
                r#"
                    delete from todo_labels where todo_id=$1
                "#
            )
            .bind(id)
            .execute(&self.pool)
            .await?;

            sqlx::query(
                r#"
                    insert into todo_labels (todo_id, label_id)
                    select $1, id
                    from unnest($2) as t(id);
                "#
            )
            .bind(id)
            .bind(labels)
            .execute(&self.pool)
            .await?;
        };

        tx.commit().await?;
        let todo = self.find(id).await?;

        Ok(todo)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let tx = self.pool.begin().await?;

        // todoラベルの削除
        sqlx::query(
            r#"
                delete from todo_labels where todo_id=$1
            "#
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        // todoの削除
        sqlx::query(
            r#"
                delete from todos where id=$1
            "#
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "database-test")]
mod test {
    use super::*;
    use dotenv::dotenv;
    use sqlx::PgPool;
    use std::env;

    #[tokio::test]
    async fn todo_crud_scenario() {
        dotenv().ok();
        let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
        let pool = PgPool::connect(database_url)
            .await
            .expect(&format!("fail connect database, url is [{}]", database_url));

        // ラベルデータの準備
        let label_name = String::from("test label");
        let optional_label = sqlx::query_as::<_, Label>(
            r#"
                select * from labels where name = $1
            "#
        )
        .bind(label_name.clone())
        .fetch_optional(&pool)
        .await
        .expect("Failed to prepare label data.");
        let label_1 = if let Some(label) = optional_label {
            label
        } else {
            let label = sqlx::query_as::<_, Label>(
                r#"
                    insert into labels ( name )
                    values ( $1 )
                    returning *
                "#,
            )
            .bind(label_name)
            .fetch_one(&pool)
            .await
            .expect("Failed to insert label data.");
            label
        };

        let repository = TodoRepositoryForDb::new(pool.clone());
        let todo_text = "[crud_scenario] text";

        // createのテスト
        let created = repository
            .create(CreateTodo::new(todo_text.to_string(), vec![label_1.id]))
            .await
            .expect("[create] returned Err");
        assert_eq!(created.text, todo_text);
        assert!(!created.completed);
        assert_eq!(*created.labels.first().unwrap(), label_1);

        // findのテスト
        let todo = repository
            .find(created.id)
            .await
            .expect("[find] returned Err");
        assert_eq!(created, todo);

        // allのテスト
        let todos = repository
            .all()
            .await
            .expect("[all] returned Err");
        let todo = todos.first().unwrap();
        assert_eq!(created, *todo);

        // updateのテスト
        let updated_text = "[crud_scenario] updated text";
        let todo = repository
            .update(
                todo.id,
                UpdateTodo {
                    text: Some(updated_text.to_string()),
                    completed: Some(true),
                    labels: Some(vec![]),
                }
            )
            .await
            .expect("[update] returned Err");
        assert_eq!(created.id, todo.id);
        assert_eq!(todo.text, updated_text);
        assert!(todo.labels.len() == 0);

        // deleteのテスト
        let _ = repository
            .delete(todo.id)
            .await
            .expect("[delete] returned Err");
        let res = repository
            .find(created.id)
            .await;
        assert!(res.is_err());

        let todo_rows = sqlx::query(
            r#"
                select * from todos where id=$1
            "#,)
            .bind(todo.id)
            .fetch_all(&pool)
            .await
            .expect("[delete] todo_labels fetch error");
        assert!(todo_rows.len() == 0);

        let rows = sqlx::query(
            r#"
                select * from todo_labels where todo_id=$1
            "#
        )
        .bind(todo.id)
        .fetch_all(&pool)
        .await
        .expect("[delete] todo_labels fetch error");
        assert!(rows.len() == 0);
    }

    #[test]
    fn fold_entities_test() {
        let label_1 = Label {
            id: 1,
            name: String::from("label 1"),
        };
        let label_2 = Label {
            id: 2,
            name: String::from("label 2")
        };
        let rows = vec![
            TodoWithLabelFromRow {
                id: 1,
                text: String::from("todo 1"),
                completed: false,
                label_id: Some(label_1.id),
                label_name: Some(label_1.name.clone()),
            },
            TodoWithLabelFromRow {
                id: 1,
                text: String::from("todo 1"),
                completed: false,
                label_id: Some(label_2.id),
                label_name: Some(label_2.name.clone()),
            },
            TodoWithLabelFromRow {
                id: 2,
                text: String::from("todo 2"),
                completed: false,
                label_id: Some(label_1.id),
                label_name: Some(label_1.name.clone()),
            }
        ];
        let res = fold_entities(rows);
        assert_eq!(
            res,
            vec![
                TodoEntity {
                    id: 1,
                    text: String::from("todo 1"),
                    completed: false,
                    labels: vec![label_1.clone(), label_2.clone()],
                },
                TodoEntity {
                    id: 2,
                    text: String::from("todo 2"),
                    completed: false,
                    labels: vec![label_1.clone()],
                }
            ]
        );
    }
}

#[cfg(test)]
pub mod test_utils {
    use anyhow::Context;
    use axum::async_trait;
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    };

    use super::*;

    impl TodoEntity {
        pub fn new(id: i32, text: String) -> Self {
            Self {
                id,
                text,
                completed: false,
                labels: vec![],
            }
        }
    }

    impl CreateTodo {
        pub fn new(text: String, labels: Vec<i32>) -> Self {
            Self { text, labels }
        }
    }

    // 仮想のデータベース（実態はHashMap）
    // テストのみで使う
    // RepositoryForMemoryでの各操作を定義する
    // このレポジトリはhttpハンドラーで使われる

    // とりあえずTODOデータをHashMapに保存する
    type TodoDatas = HashMap<i32, TodoEntity>;

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

        fn write_store_ref(&self) -> RwLockWriteGuard<TodoDatas> {
            self.store.write().unwrap()
        }

        fn read_store_ref(&self) -> RwLockReadGuard<TodoDatas> {
            self.store.read().unwrap()
        }
    }

    #[async_trait]
    impl TodoRepository for TodoRepositoryForMemory {
        async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity> {
            let mut store = self.write_store_ref();
            // 保存済みの長さ+1で新たなidを生成
            let id = (store.len() + 1) as i32;
            let todo = TodoEntity::new(id, payload.text.clone());
            store.insert(id, todo.clone());
            Ok(todo)
        }

        async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
            let store = self.read_store_ref();
            let todo = store
                .get(&id)
                .map(|todo| todo.clone())
                .ok_or(RepositoryError::NotFound(id))?;
            Ok(todo)
        }

        async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
            let store = self.read_store_ref();
            Ok(Vec::from_iter(store.values().map(|todo| todo.clone())))
        }

        async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
            let mut store = self.write_store_ref();
            let todo = store.get(&id).context(RepositoryError::NotFound(id))?;
            let text = payload.text.unwrap_or(todo.text.clone());
            let completed = payload.completed.unwrap_or(todo.completed);
            let todo = TodoEntity {
                id,
                text,
                completed,
                labels: vec![]
            };
            store.insert(id, todo.clone());
            Ok(todo)
        }

        async fn delete(&self, id: i32) -> anyhow::Result<()> {
            let mut store = self.write_store_ref();
            store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
            Ok(())
        }
    }

    mod test {
        use super::*;

        #[tokio::test]
        async fn todo_crud_scenario() {
            let text = "todo text".to_string();
            let id = 1;
            let expected = TodoEntity::new(id, text.clone());

            // create
            todo!("labelデータの追加");
            let labels = vec![];
            let repository = TodoRepositoryForMemory::new();
            let todo = repository
                .create(CreateTodo::new(text, labels))
                .await
                .expect("failed create todo");
            assert_eq!(expected, todo);

            // find
            let todo = repository.find(todo.id).await.unwrap();
            assert_eq!(expected, todo);

            // all
            let todo = repository.all().await.expect("failed get all todo");
            assert_eq!(vec![expected], todo);

            // update
            let text = "update todo text".to_string();
            let todo = repository
                .update(
                    1,
                    UpdateTodo {
                        text: Some(text.clone()),
                        completed: Some(true),
                        labels: Some(vec![])
                    },
                )
                .await
                .expect("failed update todo");
            assert_eq!(
                TodoEntity {
                    id,
                    text,
                    completed: true,
                    labels: vec![],
                },
                todo,
            );

            // delete
            let res = repository.delete(id).await;
            assert!(res.is_ok());
        }
    }
}
