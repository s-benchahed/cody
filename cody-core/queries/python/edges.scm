; Function calls
(call
  function: (identifier) @callee) @call

; Method calls
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @method)) @method_call

; Import statements
(import_statement
  name: (dotted_name) @import_path) @import

(import_from_statement
  module_name: (dotted_name) @module
  name: (dotted_name) @import_path) @from_import

; Class inheritance
(class_definition
  name: (identifier) @child
  superclasses: (argument_list
    (identifier) @parent)) @extends
