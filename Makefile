.PHONY: backend frontend minio migrate all


backend:
	cd backend-rs && cargo run

frontend:
	cd frontend && bunx --bun nuxt dev 
frontendh:
	cd frontend && bunx --bun nuxt dev --host

documentation:
	cd docs && bunx --bun nuxt dev -p 3001

update:
	cd frontend && \
	rm -rf node_modules .nuxt bun.lock && \
	bun update
	cd backend-rs && \
	cargo fmt && cargo clippy --fix --allow-dirty

minio:
	MINIO_ROOT_USER=collectanea-access MINIO_ROOT_PASSWORD=collectanea-secret \
	minio server ./minio --console-address :9001 --address "localhost:9000"

minioh:
	MINIO_ROOT_USER=collectanea-access MINIO_ROOT_PASSWORD=collectanea-secret \
	minio server ./minio --address 0.0.0.0:9000 --console-address 0.0.0.0:9001

migrate:
	migrate create -ext sql -dir backend-rs/migrations/ -seq $(name)

