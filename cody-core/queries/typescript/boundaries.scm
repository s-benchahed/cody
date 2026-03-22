; Same boundary patterns as JS — tree-sitter-typescript shares node types
(call_expression
  function: (member_expression
    object: (identifier) @obj (#match? @obj "res|response")
    property: (property_identifier) @method (#match? @method "^(setHeader|set|header)$"))
  arguments: (arguments (string) @key .)) @http_header_write

(member_expression
  object: (member_expression
    object: (identifier) @proc (#eq? @proc "process")
    property: (property_identifier) @env (#eq? @env "env"))
  property: (property_identifier) @key) @env_read

(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @method (#match? @method "^(get|hget|del|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (arguments (string) @key .)) @redis_get

(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @method (#match? @method "^(set|hset|setex|lpush|rpush|zadd|lset|expire|persist)$"))
  arguments: (arguments (string) @key .)) @redis_set

(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @method (#match? @method "^(emit|on|send)$"))
  arguments: (arguments (string) @key .)) @ws_op
