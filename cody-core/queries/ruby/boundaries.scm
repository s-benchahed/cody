; ── ENV ───────────────────────────────────────────────────────────────────────

; ENV["KEY"] / ENV.fetch("KEY") / ENV.dig("KEY")
(call
  receiver: (constant) @env (#eq? @env "ENV")
  method: (identifier) @method (#match? @method "^(\\[\\]|fetch|dig)$")
  arguments: (argument_list (string) @key .)) @env_read

; ── Redis ─────────────────────────────────────────────────────────────────────

; Redis reads: restrict to known Redis variable names to avoid false positives
; (e.g. avoid matching hash.get("key"), response.get("field"), etc.)
(call
  receiver: (identifier) @obj (#match? @obj "^(redis|cache|r|store|client|redis_client)$")
  method: (identifier) @method (#match? @method "^(get|hget|lrange|zrange|exists|ttl|rpop)$")
  arguments: (argument_list (string) @key .)) @redis_get

; Redis reads: unambiguously Redis-only methods — safe on any receiver
(call
  receiver: (identifier) @obj2
  method: (identifier) @method2 (#match? @method2 "^(hget|lrange|zrange|llen|scard|zcard|smembers|sismember|zscore|zrank|rpop)$")
  arguments: (argument_list (string) @key .)) @redis_get

; Redis writes: restrict to known Redis variable names
(call
  receiver: (identifier) @obj3 (#match? @obj3 "^(redis|cache|r|store|client|redis_client)$")
  method: (identifier) @method3 (#match? @method3 "^(set|hset|setex|lpush|rpush|zadd|lset|expire|persist|del|delete|incr|decr|setnx|psetex|sadd)$")
  arguments: (argument_list (string) @key .)) @redis_set

; Redis writes: unambiguously Redis-only methods
(call
  receiver: (identifier) @obj4
  method: (identifier) @method4 (#match? @method4 "^(hset|lpush|rpush|zadd|sadd|srem|zrem|hincrby|incrby|decrby)$")
  arguments: (argument_list (string) @key .)) @redis_set

; ── ActiveRecord (SQL) ────────────────────────────────────────────────────────

; ActiveRecord read queries: User.where(...), Post.find(id), Article.all, etc.
; Captures the model constant name as the key (maps to DB table by convention).
(call
  receiver: (constant) @key
  method: (identifier) @method (#match? @method "^(find|find_by|find_by_sql|find_or_create_by|find_or_initialize_by|first|last|all|take|where|joins|includes|eager_load|preload|order|group|having|select|count|sum|average|minimum|maximum|exists|pluck|ids|none|limit|offset|distinct|from|reorder|unscope|lock|readonly)$")) @ar_read

; ActiveRecord write queries: User.create!, Post.insert_all, etc.
(call
  receiver: (constant) @key
  method: (identifier) @method (#match? @method "^(create|create!|insert|insert!|insert_all|insert_all!|upsert|upsert_all|update_all|delete_all|destroy_all|destroy_by|delete_by)$")) @ar_write

; Raw SQL through connection.execute("SQL ...") / connection.exec_query("SQL")
(call
  receiver: (identifier) @recv (#match? @recv "^(connection|conn)$")
  method: (identifier) @method (#match? @method "^(execute|exec_query|exec_update|exec_insert|exec_delete|select_all|select_rows|select_values|select_value)$")
  arguments: (argument_list (string) @key .)) @sql_raw

; ── HTTP outbound ─────────────────────────────────────────────────────────────

; Class-level calls: Faraday.get("url"), HTTParty.post("url"), RestClient.get("url")
(call
  receiver: (constant) @obj (#match? @obj "^(Faraday|HTTParty|RestClient|HTTP|Typhoeus|Excon)$")
  method: (identifier) @method (#match? @method "^(get|post|put|delete|patch|head|request)$")
  arguments: (argument_list (string) @key .)) @http_out

; Instance calls: conn.get("/path"), @client.post("/path") — Faraday / Net::HTTP style
(call
  receiver: (identifier) @obj2 (#match? @obj2 "^(conn|connection|client|faraday|http_client|http)$")
  method: (identifier) @method2 (#match? @method2 "^(get|post|put|delete|patch)$")
  arguments: (argument_list (string) @key .)) @http_out

; ── Background jobs / Sidekiq / Active Job ────────────────────────────────────

; SomeWorker.perform_async(args) — captures the worker class name as the queue key
(call
  receiver: (constant) @key
  method: (identifier) @method (#eq? @method "perform_async")) @job_enqueue

; Active Job: queue_as :critical / queue_as "default"
(call
  method: (identifier) @fn (#eq? @fn "queue_as")
  arguments: (argument_list (simple_symbol) @key)) @job_queue

; ── Kafka ─────────────────────────────────────────────────────────────────────

; producer.produce("topic", ...) or kafka_producer.produce("topic", ...)
(call
  receiver: (identifier) @obj (#match? @obj "^(producer|kafka|kafka_producer)$")
  method: (identifier) @method (#eq? @method "produce")
  arguments: (argument_list (string) @key .)) @kafka_write

; ── HTTP response headers ─────────────────────────────────────────────────────

(call
  receiver: (identifier) @obj (#match? @obj "^(response|headers)$")
  method: (identifier) @prop (#eq? @prop "\\[\\]=")
  arguments: (argument_list (string) @key .)) @http_header_write
