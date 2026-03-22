; Function calls
(call_expression
  function: (identifier) @callee) @call

; Method calls
(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @method)) @method_call

; Import declarations
(import_statement
  source: (string) @import_path) @import

; Require calls
(call_expression
  function: (identifier) @req (#eq? @req "require")
  arguments: (arguments (string) @import_path)) @require

; Class inheritance
(class_declaration
  name: (identifier) @child
  (class_heritage
    (identifier) @parent)) @extends
