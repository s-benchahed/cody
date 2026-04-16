; Method invocations: obj.method(...)
(method_invocation
  name: (identifier) @method) @method_call

; Object creation: new ClassName(...)
(object_creation_expression
  type: (type_identifier) @callee) @new_call

; Import declarations
(import_declaration
  (scoped_identifier) @import_path) @import
