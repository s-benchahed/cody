; Method calls
(call
  method: (identifier) @callee) @call

; Chained method calls
(call
  receiver: (identifier) @obj
  method: (identifier) @method) @method_call

; require / require_relative
(call
  method: (identifier) @req (#match? @req "^(require|require_relative)$")
  arguments: (argument_list (string) @path)) @require

; Class inheritance
(class
  name: (constant) @child
  superclass: (constant) @parent) @extends
