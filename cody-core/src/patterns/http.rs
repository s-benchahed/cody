use once_cell::sync::Lazy;
use regex::Regex;

// Express: app.get('/path', handler) or router.post('/path', handler)
pub static EXPRESS_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:app|router)\.(get|post|put|delete|patch|use)\s*\(\s*['"`]([^'"`]+)['"`]\s*,\s*(\w+)"#).unwrap()
});

// FastAPI / Flask: @app.get("/path") / @router.post("/path")
pub static FASTAPI_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"@(?:app|router|blueprint)\.(get|post|put|delete|patch|route)\s*\(\s*['"]([^'"]+)['"]\s*\)[\s\S]{0,100}?(?:async\s+)?def\s+(\w+)"#).unwrap()
});

// Celery task: @app.task / @celery.task / @shared_task
pub static CELERY_TASK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"@(?:app\.task|celery\.task|shared_task)[^\n]*\n(?:async\s+)?def\s+(\w+)"#).unwrap()
});

// Rails routes: get '/path', to: 'controller#action'
pub static RAILS_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|\s)(get|post|put|delete|patch|resources?)\s+["']([^"']+)["']\s*,\s*to:\s*["']([^"']+)["']"#).unwrap()
});

// Rust Actix/Rocket: #[get("/path")] async fn handler
pub static RUST_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"#\[(get|post|put|delete|patch)\s*\(\s*"([^"]+)"\s*\)\s*\]\s*(?:async\s+)?(?:pub\s+)?fn\s+(\w+)"#).unwrap()
});

// res.setHeader('X-Foo', ...) — regex fallback for JS
pub static SETHEADER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"res\.(setHeader|set)\s*\(\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// req.headers['X-Foo'] — regex fallback
pub static REQ_HEADER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"req\.headers\[['"`]([^'"`]+)['"`]\]|req\.headers\.get\s*\(\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// Python requests: response.headers['X-Foo']
pub static PY_RESP_HEADER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:response|resp)\.headers\[['"]([^'"]+)['"]\]"#).unwrap()
});

// Axum builder: .route("/path", get(handler))
pub static AXUM_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    // Capture full module paths like services::pages::handlers::get_fact or simple identifiers
    Regex::new(r#"\.route\s*\(\s*"([^"]+)"\s*,\s*(get|post|put|delete|patch|head|options)\s*\(\s*([\w:]+)\s*\)"#).unwrap()
});

// Axum with middleware wrapper: .route("/path", with_lp_auth(post(handlers::fn), ...))
// Groups: 1=path, 2=wrapper_fn, 3=method, 4=handler
pub static AXUM_WRAPPED_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\.route\s*\(\s*"([^"]+)"\s*,\s*(\w+)\s*\(\s*(get|post|put|delete|patch|head|options)\s*\(\s*([\w:]+)\s*\)"#).unwrap()
});

// Fastify: fastify.get('/path', handler) or fastify.post(...)
pub static FASTIFY_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:fastify|server|app)\.(get|post|put|delete|patch)\s*\(\s*['"`]([^'"`]+)['"`]\s*,\s*(?:async\s+)?(?:function\s+)?(\w+)"#).unwrap()
});

// Django urlpatterns: path('url/', view_fn) or re_path(r'...', view_fn)
pub static DJANGO_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:path|re_path)\s*\(\s*r?['"]([^'"]+)['"]\s*,\s*(\w+)"#).unwrap()
});

// Rust/Axum: fn handler(user: LpAuthUser, State(s): State<AppState>, ...) — extractor types in params
pub static RUST_EXTRACTOR_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?:async\s+)?(?:pub\s+)?fn\s+(\w+)\s*\((?:[^)]*\b(?:State|Extension|AuthUser|LpAuthUser|CurrentUser|AdminUser|Claims|JwtClaims)\b[^)]*)\)"#
    ).unwrap()
});

// Rust: also capture the extractor type names from the same function
pub static RUST_EXTRACTOR_TYPES_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\b(State|Extension|AuthUser|LpAuthUser|CurrentUser|AdminUser|Claims|JwtClaims)\b"#).unwrap()
});

// Python/FastAPI: Depends(some_func) in function signature
pub static FASTAPI_DEPENDS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:async\s+)?def\s+(\w+)\s*\([^)]*Depends\s*\(\s*(\w+)\s*\)[^)]*\)"#).unwrap()
});

// Python: Security(func) annotation
pub static FASTAPI_SECURITY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Security\s*\(\s*(\w+)\s*\)"#).unwrap()
});

// Ruby/Rails: before_action :method_name or before_action :method_name, only: [...]
pub static RAILS_BEFORE_ACTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"before_action\s+:(\w+)"#).unwrap()
});

// TypeScript/NestJS: @UseGuards(GuardName)
pub static NESTJS_GUARD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"@UseGuards\s*\(\s*(\w+)"#).unwrap()
});

// Express: router.use(middleware, handler) — capture middleware name when used inline before handler
pub static EXPRESS_USE_MW_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:router|app)\.use\s*\(\s*['"`][^'"`]*['"`]\s*,\s*(\w+)\s*,"#).unwrap()
});
