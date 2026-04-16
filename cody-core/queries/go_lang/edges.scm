; Direct function calls
(call_expression
  function: (identifier) @callee) @call

; Method / selector calls
(call_expression
  function: (selector_expression
    field: (field_identifier) @method)) @method_call

; Import declarations
(import_declaration
  (import_spec
    path: (interpreted_string_literal) @import_path)) @import
