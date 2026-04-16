; Function call: foo(...)
(function_call_expression
  function: (name) @callee) @call

; Method call: $obj->method(...)
(member_call_expression
  name: (name) @method) @method_call

; Static call: ClassName::method(...)
(scoped_call_expression
  name: (name) @method) @static_call

; Use/import
(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @import_path)) @import
