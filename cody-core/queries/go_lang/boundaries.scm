; ── Environment ───────────────────────────────────────────────────────────────

; os.Getenv("KEY")
(call_expression
  function: (selector_expression
    operand: (identifier) @pkg (#eq? @pkg "os")
    field: (field_identifier) @fn (#eq? @fn "Getenv"))
  arguments: (argument_list (interpreted_string_literal) @key)) @env_read

; ── SQL (database/sql) ────────────────────────────────────────────────────────

; db.Query("SQL") / db.Exec("SQL") / db.QueryRow("SQL") etc.
(call_expression
  function: (selector_expression
    operand: (identifier) @obj
    field: (field_identifier) @method (#match? @method "^(Query|QueryContext|Exec|ExecContext|QueryRow|QueryRowContext|Prepare|PrepareContext)$"))
  arguments: (argument_list (interpreted_string_literal) @key .)) @sql_query

; ── Redis (go-redis v8/v9) ────────────────────────────────────────────────────

; rdb.Get(ctx, "key") / client.HGet(ctx, "key") etc.
(call_expression
  function: (selector_expression
    operand: (identifier) @obj (#match? @obj "^(rdb|client|redis|cache|redisClient)$")
    field: (field_identifier) @method (#match? @method "^(Get|HGet|LRange|ZRange|Exists|TTL)$"))
  arguments: (argument_list _ (interpreted_string_literal) @key .)) @redis_get

; rdb.Set(ctx, "key", ...) / client.HSet(ctx, "key", ...) etc.
(call_expression
  function: (selector_expression
    operand: (identifier) @obj (#match? @obj "^(rdb|client|redis|cache|redisClient)$")
    field: (field_identifier) @method (#match? @method "^(Set|HSet|SetEX|LPush|RPush|ZAdd|Expire)$"))
  arguments: (argument_list _ (interpreted_string_literal) @key .)) @redis_set

; ── HTTP routes (net/http standard library + gorilla/mux) ─────────────────────

; http.HandleFunc("/path", handler) / mux.Handle("/path", handler)
(call_expression
  function: (selector_expression
    operand: (identifier) @pkg (#match? @pkg "^(http|mux|router|r)$")
    field: (field_identifier) @fn (#match? @fn "^(HandleFunc|Handle)$"))
  arguments: (argument_list (interpreted_string_literal) @path .)) @route

; ── Gin / chi / echo / fiber routes ──────────────────────────────────────────

; r.GET("/path", handler) — gin, chi (r.Get), echo (e.GET), fiber (app.Get)
(call_expression
  function: (selector_expression
    operand: (identifier) @obj (#match? @obj "^(r|router|e|app|v1|v2|api|g)$")
    field: (field_identifier) @method (#match? @method "^(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|Get|Post|Put|Delete|Patch)$"))
  arguments: (argument_list (interpreted_string_literal) @path .)) @route

; ── gRPC (protobuf decode) ────────────────────────────────────────────────────

; proto.Unmarshal(data, &SomeMsg{}) — captures type name from composite literal
(call_expression
  function: (selector_expression
    operand: (identifier) @pkg (#match? @pkg "^(proto|protojson|encoding)$")
    field: (field_identifier) @fn (#match? @fn "^(Unmarshal|UnmarshalText|UnmarshalJSON)$"))
  arguments: (argument_list _ (unary_expression (composite_literal type: (type_identifier) @key)))) @grpc_decode
