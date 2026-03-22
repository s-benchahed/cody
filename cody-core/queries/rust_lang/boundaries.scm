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

; Redis reads: get, hget, del
(call_expression
  function: (field_expression
    field: (field_identifier) @method (#match? @method "^(get|hget|del|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (arguments (string_literal) @key .)) @redis_get

; Redis writes: set, hset, set_ex, lpush
(call_expression
  function: (field_expression
    field: (field_identifier) @method (#match? @method "^(set|hset|set_ex|lpush|rpush|zadd|lset|expire|persist)$"))
  arguments: (arguments (string_literal) @key .)) @redis_set

; HTTP route attribute macros (Actix/Rocket)
(attribute_item
  (attribute
    (identifier) @method (#match? @method "^(get|post|put|delete|patch|route)$")
    arguments: (token_tree (string_literal) @path))) @route
