; os.environ.get / os.getenv
(call
  function: (attribute
    object: (attribute
      object: (identifier) @os (#eq? @os "os")
      attribute: (identifier) @env (#eq? @env "environ"))
    attribute: (identifier) @method (#eq? @method "get"))
  arguments: (argument_list (string) @key .)) @env_read

(call
  function: (attribute
    object: (identifier) @os (#eq? @os "os")
    attribute: (identifier) @fn (#eq? @fn "getenv"))
  arguments: (argument_list (string) @key .)) @env_read2

; redis reads: get, hget, delete, rpop
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @method (#match? @method "^(get|hget|delete|rpop|lrange|zrange|exists|ttl)$"))
  arguments: (argument_list (string) @key .)) @redis_get

; redis writes: set, hset, setex, lpush
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @method (#match? @method "^(set|hset|setex|lpush|rpush|zadd|lset|expire|persist)$"))
  arguments: (argument_list (string) @key .)) @redis_set

; requests.get / requests.post (HTTP)
(call
  function: (attribute
    object: (identifier) @lib (#match? @lib "^(requests|session|client)$")
    attribute: (identifier) @method (#match? @method "^(get|post|put|delete|patch)$"))
  arguments: (argument_list (string) @url .)) @http_call

; Flask/FastAPI route decorator
(decorated_definition
  (decorator
    (call
      function: (attribute
        attribute: (identifier) @method (#match? @method "^(get|post|put|delete|patch|route)$"))
      arguments: (argument_list (string) @route_path .)))
  definition: (function_definition name: (identifier) @handler)) @route

; Kafka producer
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @method (#eq? @method "produce"))
  arguments: (argument_list (string) @topic .)) @kafka_write

; response.set_cookie
(call
  function: (attribute
    object: (identifier) @obj (#match? @obj "^(response|resp)$")
    attribute: (identifier) @method (#eq? @method "set_cookie"))
  arguments: (argument_list (string) @key .)) @cookie_write
