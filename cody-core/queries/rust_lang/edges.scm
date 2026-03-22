; Function calls
(call_expression
  function: (identifier) @callee) @call

; Method calls
(call_expression
  function: (field_expression
    field: (field_identifier) @method)) @method_call

; Use declarations
(use_declaration
  argument: (scoped_identifier
    path: (identifier) @crate
    name: (identifier) @item)) @use

; Struct implementation
(impl_item
  type: (type_identifier) @type) @impl
