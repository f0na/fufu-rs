# worker-rs-axum Skill

A reference skill for building Cloudflare Workers with Rust (worker-rs) and axum framework.

## Usage

This skill is automatically invoked when you're building:
- Cloudflare Workers projects with Rust
- WASM-based serverless APIs
- JWT authentication with D1 database
- RESTful API patterns in worker-rs

## Quick Start

### 1. Prerequisites

```bash
# Add wasm32 target
rustup target add wasm32-unknown-unknown

# Install worker-build
cargo install worker-build

# Install wrangler
npm install -g wrangler
```

### 2. Create D1 Database

```bash
wrangler d1 create project-db
```

Copy the database_id to wrangler.toml.

### 3. Initialize Schema

```bash
wrangler d1 execute project-db --file=./schema.sql
```

### 4. Development

```bash
wrangler dev
```

### 5. Deploy

```bash
wrangler deploy
```

## Templates

The `templates/` directory contains ready-to-use code files:

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust dependencies |
| `wrangler.toml` | Workers configuration |
| `schema.sql` | D1 database schema |
| `lib.rs` | WASM entry point |
| `router.rs` | Axum router definition |
| `error.rs` | Error handling |
| `db.rs` | Database helper |
| `auth/*.rs` | JWT authentication |
| `handlers/*.rs` | Route handlers |
| `models/*.rs` | Data models |

## Key Patterns

### Protected Routes

Use `Claims` extractor to require authentication:

```rust
#[worker::send]
pub async fn protected_route(
    claims: Claims,  // Auto-validates JWT
    State(env): State<Arc<Env>>,
) -> Result<Json<Response>, AppError> {
    // claims.sub = user_id
}
```

### D1 Database Queries

```rust
// Query all
let results = db.prepare("SELECT * FROM table")
    .all().await?;
let items = results.results::<Value>()?;

// Query single
let row = db.prepare("SELECT * FROM table WHERE id = ?1")
    .bind(&[id.into()])?
    .first::<Item>(None).await?;

// Insert/Update
db.prepare("INSERT INTO table VALUES (?1, ?2)")
    .bind(&[a.into(), b.into()])?
    .run().await?;
```

## References

- [worker-rs documentation](https://github.com/cloudflare/workers-rs)
- [axum documentation](https://docs.rs/axum)
- [Cloudflare D1 documentation](https://developers.cloudflare.com/d1/)