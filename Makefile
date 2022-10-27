include .env

build:
	docker-compose build

db:
	docker-compose up

dev:
	sqlx db create --database-url $(DATABASE_URL)
	sqlx migrate run
	cargo watch -x run

test:
	cargo test

# standalone test
test-s:
	cargo test --no-default-features
