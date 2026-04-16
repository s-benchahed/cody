; ── Environment ────────────────────────────────────────────────────────────────

; getenv("KEY") or env("KEY") (Laravel helper)
(function_call_expression
  function: (name) @fn (#match? @fn "^(getenv|env)$")
  arguments: (arguments (argument (string) @key .))) @env_read

; ── SQL (Laravel DB facade + PDO) ──────────────────────────────────────────────

; DB::select("SQL") / DB::insert / DB::update / DB::delete / DB::statement
(scoped_call_expression
  scope: (name) @obj (#eq? @obj "DB")
  name: (name) @method (#match? @method "^(select|insert|update|delete|statement|unprepared)$")
  arguments: (arguments (argument (string) @key .))) @sql_query

; DB::table("tablename")
(scoped_call_expression
  scope: (name) @obj2 (#eq? @obj2 "DB")
  name: (name) @method2 (#eq? @method2 "table")
  arguments: (arguments (argument (string) @key .))) @sql_table

; $pdo->query("SQL") / $pdo->prepare("SQL") / $pdo->exec("SQL")
(member_call_expression
  object: (variable_name (name) @obj3 (#match? @obj3 "^(pdo|db|conn|connection)$"))
  name: (name) @method3 (#match? @method3 "^(query|prepare|exec)$")
  arguments: (arguments (argument (string) @key .))) @sql_query

; ── Redis (Laravel Redis facade / Predis) ──────────────────────────────────────

; Redis::get("key") / Cache::get("key")
(scoped_call_expression
  scope: (name) @obj (#match? @obj "^(Redis|Cache)$")
  name: (name) @method (#match? @method "^(get|hget|lrange|exists|ttl)$")
  arguments: (arguments (argument (string) @key .))) @redis_get

; Redis::set("key", ...) / Cache::put("key", ...)
(scoped_call_expression
  scope: (name) @obj2 (#match? @obj2 "^(Redis|Cache)$")
  name: (name) @method2 (#match? @method2 "^(set|hset|setex|lpush|rpush|zadd|put|forever)$")
  arguments: (arguments (argument (string) @key .))) @redis_set

; $redis->get("key") / $cache->get("key")
(member_call_expression
  object: (variable_name (name) @var (#match? @var "^(redis|cache|predis|client)$"))
  name: (name) @method3 (#match? @method3 "^(get|hget|lrange)$")
  arguments: (arguments (argument (string) @key .))) @redis_get

; $redis->set("key", ...) / $cache->put("key", ...)
(member_call_expression
  object: (variable_name (name) @var2 (#match? @var2 "^(redis|cache|predis|client)$"))
  name: (name) @method4 (#match? @method4 "^(set|hset|setex|lpush|rpush|put)$")
  arguments: (arguments (argument (string) @key .))) @redis_set

; ── HTTP outbound (Laravel Http / Guzzle) ──────────────────────────────────────

; Http::get("url") / Http::post("url") — Laravel Http facade
(scoped_call_expression
  scope: (name) @obj (#eq? @obj "Http")
  name: (name) @method (#match? @method "^(get|post|put|delete|patch)$")
  arguments: (arguments (argument (string) @key .))) @http_out

; $client->request("GET", "url") / $client->get("url") — Guzzle
(member_call_expression
  object: (variable_name (name) @obj2 (#match? @obj2 "^(client|http|httpClient|guzzle)$"))
  name: (name) @method2 (#match? @method2 "^(get|post|put|delete|request|send)$")
  arguments: (arguments (argument (string) @key .))) @http_out

; ── Routes (Laravel Route facade) ──────────────────────────────────────────────

; Route::get("/path", [Controller::class, "method"]) or Route::get("/path", "handler")
(scoped_call_expression
  scope: (name) @obj (#eq? @obj "Route")
  name: (name) @method (#match? @method "^(get|post|put|delete|patch|any|match|resource|apiResource)$")
  arguments: (arguments (argument (string) @path) .)) @route

; ── Kafka (optional) ───────────────────────────────────────────────────────────

; $producer->produce($topic, ...) or $kafka->send("topic", ...)
(member_call_expression
  object: (variable_name (name) @obj (#match? @obj "^(producer|kafka|kafkaProducer)$"))
  name: (name) @method (#match? @method "^(produce|send|publish)$")
  arguments: (arguments (argument (string) @key .))) @kafka_write
