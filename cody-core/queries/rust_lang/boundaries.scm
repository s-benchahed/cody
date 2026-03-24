; std::env::var("KEY")
(call_expression
  function: (scoped_identifier
    path: (scoped_identifier) @path (#match? @path "env")
    name: (identifier) @fn (#eq? @fn "var"))
  arguments: (arguments (string_literal) @key)) @env_read

; sqlx query
(macro_invocation
  macro: (identifier) @mac (#match? @mac "^(query|query_as|sqlx)$")
  (token_tree (string_literal) @sql)) @sql_query

; Redis reads: conn.get("key") — receiver must be plain identifier (not a method chain)
(call_expression
  function: (field_expression
    value: (identifier)
    field: (field_identifier) @method (#match? @method "^(get|hget|del|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (arguments (string_literal) @key .)) @redis_get

; Redis reads: self.redis.get("key") — receiver is a field access
(call_expression
  function: (field_expression
    value: (field_expression)
    field: (field_identifier) @method (#match? @method "^(get|hget|del|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (arguments (string_literal) @key .)) @redis_get

; Redis writes: conn.set("key", val) — receiver must be plain identifier
(call_expression
  function: (field_expression
    value: (identifier)
    field: (field_identifier) @method (#match? @method "^(set|hset|set_ex|lpush|rpush|zadd|lset|expire|persist)$"))
  arguments: (arguments (string_literal) @key .)) @redis_set

; Redis writes: self.redis.set("key", val) — receiver is a field access
(call_expression
  function: (field_expression
    value: (field_expression)
    field: (field_identifier) @method (#match? @method "^(set|hset|set_ex|lpush|rpush|zadd|lset|expire|persist)$"))
  arguments: (arguments (string_literal) @key .)) @redis_set

; HTTP route attribute macros (Actix/Rocket)
(attribute_item
  (attribute
    (identifier) @method (#match? @method "^(get|post|put|delete|patch|route)$")
    arguments: (token_tree (string_literal) @path))) @route
