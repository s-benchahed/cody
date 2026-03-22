use once_cell::sync::Lazy;
use regex::Regex;

// Extract table names from SQL string literals
pub static SQL_FROM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(?:FROM|JOIN|UPDATE|INTO|TABLE)\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).unwrap()
});

// Raw SQL query call: db.query("...") / conn.query("...") / pool.query("...")
pub static DB_QUERY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:db|conn|pool|client)\.(query|execute|run|raw)\s*\(\s*['"`]([^'"`]{5,})"#).unwrap()
});

// Prisma ORM: prisma.tableName.findMany / create / update / delete
pub static PRISMA_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"prisma\.([a-zA-Z_][a-zA-Z0-9_]*)\.(findMany|findFirst|findUnique|create|update|upsert|delete|deleteMany|updateMany|count)"#).unwrap()
});

// Django ORM: Model.objects.filter / get / all / create / save
pub static DJANGO_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"([A-Z][a-zA-Z0-9_]*)\.objects\.(filter|get|all|create|update|delete|bulk_create|first|last)"#).unwrap()
});

// SQLAlchemy: session.query(Model) / session.add(obj)
pub static SQLALCHEMY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:session|db\.session)\.(query|add|delete|merge|flush|commit)\s*\(\s*([A-Z][a-zA-Z0-9_]*)"#).unwrap()
});

// GORM: db.Where(...).Find(&result) / db.Create(&model)
pub static GORM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"db\.(Create|Save|First|Find|Delete|Update|Table)\s*\(&?([A-Z][a-zA-Z0-9_]*)"#).unwrap()
});

// sqlx macro: query!("SELECT ... FROM table")
pub static SQLX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:query|query_as)!\s*\(\s*"([^"]{5,})"#).unwrap()
});

/// Extract table names from a SQL string
pub fn extract_tables(sql: &str) -> Vec<String> {
    SQL_FROM_RE.captures_iter(sql)
        .filter_map(|cap| cap.get(1))
        .map(|m| m.as_str().to_lowercase())
        .filter(|t| !RESERVED.contains(&t.as_str()))
        .collect()
}

const RESERVED: &[&str] = &[
    "select", "from", "where", "join", "on", "and", "or",
    "null", "true", "false", "not", "in", "as", "by",
];
