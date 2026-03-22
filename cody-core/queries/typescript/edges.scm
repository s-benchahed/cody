; Same as JS
(call_expression
  function: (identifier) @callee) @call

(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @method)) @method_call

(import_statement
  source: (string) @import_path) @import

(class_declaration
  name: (type_identifier) @child
  (class_heritage
    (extends_clause
      value: (identifier) @parent))) @extends

; TypeScript implements
(class_declaration
  name: (type_identifier) @child
  (class_heritage
    (implements_clause
      (type_identifier) @iface))) @implements
