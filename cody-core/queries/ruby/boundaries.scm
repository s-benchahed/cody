; ENV access
(call
  receiver: (constant) @env (#eq? @env "ENV")
  method: (identifier) @method (#match? @method "^(\\[\\]|fetch|dig)$")
  arguments: (argument_list (string) @key .)) @env_read

; Redis reads: get, hget, del, rpop
(call
  receiver: (identifier) @obj
  method: (identifier) @method (#match? @method "^(get|hget|del|rpop|lrange|zrange|exists|ttl)$")
  arguments: (argument_list (string) @key .)) @redis_get

; Redis writes: set, hset, setex, lpush
(call
  receiver: (identifier) @obj
  method: (identifier) @method (#match? @method "^(set|hset|setex|lpush|rpush|zadd|lset|expire|persist)$")
  arguments: (argument_list (string) @key .)) @redis_set

; Rails HTTP response headers
(call
  receiver: (identifier) @obj (#match? @obj "^(response|headers)$")
  method: (identifier) @prop (#eq? @prop "\\[\\]=")
  arguments: (argument_list (string) @key .)) @http_header_write

; Sidekiq/job route-like perform_async
(call
  method: (identifier) @method (#eq? @method "perform_async")) @job_enqueue
