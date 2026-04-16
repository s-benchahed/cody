; System.getenv("KEY")
(method_invocation
  object: (identifier) @obj (#eq? @obj "System")
  name: (identifier) @fn (#eq? @fn "getenv")
  arguments: (argument_list (string_literal) @key)) @env_read

; jdbcTemplate.query("SQL", ...) / jdbcTemplate.update("SQL", ...) / jdbcTemplate.execute("SQL")
(method_invocation
  object: (identifier) @obj (#match? @obj "^(jdbcTemplate|jdbc|db|conn|connection|namedJdbc)$")
  name: (identifier) @method (#match? @method "^(query|queryForObject|queryForList|queryForMap|update|execute|batchUpdate)$")
  arguments: (argument_list (string_literal) @key .)) @sql_query

; entityManager.createQuery("SQL") / createNativeQuery("SQL")
(method_invocation
  object: (identifier) @obj2 (#match? @obj2 "^(entityManager|em|session)$")
  name: (identifier) @method2 (#match? @method2 "^(createQuery|createNativeQuery|createNamedQuery)$")
  arguments: (argument_list (string_literal) @key .)) @sql_query

; jedis.get("key") / redisTemplate reads
(method_invocation
  object: (identifier) @obj (#match? @obj "^(jedis|redis|redisClient|valueOps|hashOps|listOps)$")
  name: (identifier) @method (#match? @method "^(get|hget|lrange|exists|ttl)$")
  arguments: (argument_list (string_literal) @key .)) @redis_get

; jedis.set("key", ...) / redisTemplate writes
(method_invocation
  object: (identifier) @obj (#match? @obj "^(jedis|redis|redisClient|valueOps|hashOps|listOps)$")
  name: (identifier) @method (#match? @method "^(set|hset|setex|lpush|rpush|zadd|expire)$")
  arguments: (argument_list (string_literal) @key .)) @redis_set

; kafkaTemplate.send("topic", payload)
(method_invocation
  object: (identifier) @obj (#match? @obj "^(kafkaTemplate|producer|kafka)$")
  name: (identifier) @fn (#eq? @fn "send")
  arguments: (argument_list (string_literal) @key .)) @kafka_write

; restTemplate.getForObject("url", ...) / postForObject / exchange
(method_invocation
  object: (identifier) @obj (#match? @obj "^(restTemplate|client|httpClient|webClient)$")
  name: (identifier) @method (#match? @method "^(getForObject|getForEntity|postForObject|postForEntity|exchange|execute)$")
  arguments: (argument_list (string_literal) @key .)) @http_out

; @GetMapping("/path") / @PostMapping / @PutMapping / @DeleteMapping / @PatchMapping / @RequestMapping
(annotation
  name: (identifier) @ann (#match? @ann "^(GetMapping|PostMapping|PutMapping|DeleteMapping|PatchMapping|RequestMapping)$")
  arguments: (annotation_argument_list (string_literal) @key)) @route

; @KafkaListener(topics = "topic-name")
(annotation
  name: (identifier) @ann2 (#eq? @ann2 "KafkaListener")
  arguments: (annotation_argument_list
    (element_value_pair
      key: (identifier) @k (#eq? @k "topics")
      value: (string_literal) @key))) @kafka_listen
