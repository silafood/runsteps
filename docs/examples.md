# runsteps Example Configs

## 1. Kubernetes Deployment Pipeline

```toml
[metadata]
name = "k8s-deploy"
description = "Kubernetes deployment pipeline"
justfile = "./justfile"
working_directory = "."

[[steps]]
name = "add-helm-repo"
description = "Add and update Helm chart repositories"
command = "helm repo add bitnami https://charts.bitnami.com/bitnami && helm repo update"
group = "setup"

[[steps]]
name = "create-namespace"
description = "Create Kubernetes namespace if it does not exist"
command = "kubectl create namespace myapp --dry-run=client -o yaml | kubectl apply -f -"
group = "setup"

[[steps]]
name = "install-crds"
description = "Install Custom Resource Definitions"
just_recipe = "install-crds"
group = "setup"
depends_on = ["add-helm-repo"]

[[steps]]
name = "deploy-postgres"
description = "Deploy PostgreSQL via Helm"
command = "helm upgrade --install postgres bitnami/postgresql -n myapp -f values/postgres.yaml"
group = "deploy"
depends_on = ["create-namespace", "add-helm-repo"]

[[steps]]
name = "deploy-app"
description = "Deploy application via Helm"
just_recipe = "deploy"
group = "deploy"
confirm = true
depends_on = ["deploy-postgres", "install-crds"]

[[steps]]
name = "smoke-test"
description = "Run smoke tests against deployed services"
command = "kubectl rollout status deployment/myapp -n myapp --timeout=120s"
group = "verify"
depends_on = ["deploy-app"]
```

## 2. Rust Project Release Workflow

```toml
[metadata]
name = "rust-release"
description = "Rust project release workflow"
working_directory = "."

[[steps]]
name = "fmt-check"
description = "Verify code formatting"
command = "cargo fmt --check"
group = "ci"

[[steps]]
name = "clippy"
description = "Run Clippy lints (warnings as errors)"
command = "cargo clippy -- -D warnings"
group = "ci"

[[steps]]
name = "test"
description = "Run all tests"
command = "cargo test"
group = "ci"
depends_on = ["fmt-check", "clippy"]

[[steps]]
name = "build-release"
description = "Build release binary"
command = "cargo build --release"
group = "build"
depends_on = ["test"]

[[steps]]
name = "bump-version"
description = "Bump version in Cargo.toml (edit manually, then run this)"
command = "cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version'"
group = "release"

[[steps]]
name = "publish"
description = "Publish crate to crates.io"
command = "cargo publish"
group = "release"
confirm = true
depends_on = ["build-release", "bump-version"]
```

## 3. Docker Compose Service Management

```toml
[metadata]
name = "docker-services"
description = "Docker Compose service management"
working_directory = "."

[[steps]]
name = "pull-images"
description = "Pull latest Docker images"
command = "docker compose pull"
group = "setup"

[[steps]]
name = "build-images"
description = "Build local Docker images"
command = "docker compose build --no-cache"
group = "setup"

[[steps]]
name = "start-db"
description = "Start database service"
command = "docker compose up -d db"
group = "services"
depends_on = ["pull-images"]

[[steps]]
name = "run-migrations"
description = "Run database migrations"
command = "docker compose run --rm app migrate"
group = "services"
depends_on = ["start-db"]

[[steps]]
name = "start-app"
description = "Start the application"
command = "docker compose up -d app"
group = "services"
depends_on = ["run-migrations", "build-images"]

[[steps]]
name = "teardown"
description = "Stop and remove all containers and volumes"
command = "docker compose down -v"
group = "cleanup"
confirm = true
```

## 4. Shell-Only Workflow (No just Required)

```toml
[metadata]
name = "data-pipeline"
description = "Data processing pipeline — shell commands only"
working_directory = "/data/pipeline"

[[steps]]
name = "validate-input"
description = "Validate input files exist and are non-empty"
command = "test -s input/raw.csv && echo 'Input OK'"
group = "validate"

[[steps]]
name = "clean-data"
description = "Remove duplicate rows and normalize CSV"
command = "sort -u input/raw.csv > work/cleaned.csv"
group = "process"
depends_on = ["validate-input"]

[[steps]]
name = "transform"
description = "Apply transformation script"
command = "python3 scripts/transform.py work/cleaned.csv work/transformed.csv"
group = "process"
depends_on = ["clean-data"]

[[steps]]
name = "load"
description = "Load transformed data into database"
command = "psql $DATABASE_URL -c \"\\copy staging.data FROM 'work/transformed.csv' CSV HEADER\""
group = "load"
confirm = true
depends_on = ["transform"]

[[steps]]
name = "report"
description = "Generate summary report"
command = "python3 scripts/report.py > output/report.html"
group = "output"
depends_on = ["load"]
```
