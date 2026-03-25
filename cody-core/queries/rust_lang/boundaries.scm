; std::env::var("KEY")
(call_expression
  function: (scoped_identifier
    path: (scoped_identifier) @path (#match? @path "env")
    name: (identifier) @fn (#eq? @fn "var"))
  arguments: (arguments (string_literal) @key)) @env_read

; sqlx::query("SQL ...") — function call with regular string literal
(call_expression
  function: (scoped_identifier
    path: (identifier) @pkg (#eq? @pkg "sqlx")
    name: (identifier) @fn (#match? @fn "^(query|query_as|query_scalar|query_file|query_file_as)$"))
  arguments: (arguments (string_literal) @sql)) @sql_query

; sqlx::query(r#"SQL ..."#) — function call with raw string literal
(call_expression
  function: (scoped_identifier
    path: (identifier) @pkg2 (#eq? @pkg2 "sqlx")
    name: (identifier) @fn2 (#match? @fn2 "^(query|query_as|query_scalar|query_file|query_file_as)$"))
  arguments: (arguments (raw_string_literal) @sql)) @sql_query

; sqlx::query! macro style (regular string)
(macro_invocation
  macro: (scoped_identifier
    path: (identifier) @pkg3 (#eq? @pkg3 "sqlx")
    name: (identifier) @mac (#match? @mac "^(query|query_as|query_scalar)$"))
  (token_tree (string_literal) @sql)) @sql_query

; sqlx::query! macro style (raw string)
(macro_invocation
  macro: (scoped_identifier
    path: (identifier) @pkg4 (#eq? @pkg4 "sqlx")
    name: (identifier) @mac2 (#match? @mac2 "^(query|query_as|query_scalar)$"))
  (token_tree (raw_string_literal) @sql)) @sql_query

; sqlx bare macro: query!("SQL"), query_as!(Type, "SQL")
(macro_invocation
  macro: (identifier) @mac3 (#match? @mac3 "^(query|query_as|query_scalar)$")
  (token_tree (string_literal) @sql)) @sql_query

; sqlx bare macro with raw string: query!(r#"SQL"#)
(macro_invocation
  macro: (identifier) @mac4 (#match? @mac4 "^(query|query_as|query_scalar)$")
  (token_tree (raw_string_literal) @sql)) @sql_query

; Redis reads: conn.hget/del/rpop/lrange/zrange/exists/ttl(key) — any identifier receiver
; Note: `get` is intentionally excluded here to avoid false positives with sqlx row.get(),
; serde_json value.get(), HashMap::get(), etc. See the conn-restricted pattern below.
(call_expression
  function: (field_expression
    value: (identifier)
    field: (field_identifier) @method (#match? @method "^(hget|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (arguments (_) @key .)) @redis_get

; Redis reads: conn.get(key) — receiver must look like a Redis connection or HTTP headers map.
; "headers" is included so that headers.get("x-timezone") is captured and reclassified as
; http_header by is_http_header_name() in rust_lang.rs.
(call_expression
  function: (field_expression
    value: (identifier) @recv (#match? @recv "(conn|redis|headers)")
    field: (field_identifier) @method (#eq? @method "get"))
  arguments: (arguments (_) @key .)) @redis_get

; Redis reads: self.redis.get(key) / self.redis_conn.get(key)
; Require inner field to contain "conn" or "redis" to avoid matching self.cache.get() / self.items.get() etc.
(call_expression
  function: (field_expression
    value: (field_expression
      field: (field_identifier) @inner_field (#match? @inner_field "(conn|redis)"))
    field: (field_identifier) @method (#match? @method "^(get|hget|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (arguments (_) @key .)) @redis_get

; Redis writes: conn.set/set_ex/hset/lpush/rpush/zadd/lset/expire/persist/del(key, ...)
(call_expression
  function: (field_expression
    value: (identifier)
    field: (field_identifier) @method (#match? @method "^(set|set_ex|hset|lpush|rpush|zadd|lset|expire|persist|del)$"))
  arguments: (arguments (_) @key .)) @redis_set

(call_expression
  function: (field_expression
    value: (field_expression
      field: (field_identifier) @inner_field2 (#match? @inner_field2 "(conn|redis)"))
    field: (field_identifier) @method (#match? @method "^(set|set_ex|hset|lpush|rpush|zadd|lset|expire|persist|del)$"))
  arguments: (arguments (_) @key .)) @redis_set

; HTTP route attribute macros (Actix/Rocket)
(attribute_item
  (attribute
    (identifier) @method (#match? @method "^(get|post|put|delete|patch|route)$")
    arguments: (token_tree (string_literal) @path))) @route
